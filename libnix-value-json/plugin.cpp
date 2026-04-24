#include <cmath>
#include <cstdio>
#include <string>
#include <string_view>
#include <unordered_map>
#include <utility>
#include <vector>

#include "nix/expr/eval.hh"
#include "nix/expr/primops.hh"
#include "nix/expr/value.hh"

using namespace nix;

namespace {

constexpr std::size_t kMaxDepth = 64;

struct SeenState {
  std::unordered_map<const void *, std::string> active;
  std::unordered_map<const void *, std::string> seen;
};

using PathParts = std::vector<std::string>;
using PathFilters = std::vector<PathParts>;
using AttrPath = std::vector<std::string_view>;

static void append_json_control_char_escape(std::string &out, unsigned char c) {
  static const char *hex = "0123456789abcdef";
  out += "\\u00";
  out += hex[(c >> 4) & 0x0f];
  out += hex[c & 0x0f];
}

[[noreturn]] static void throw_invalid_utf8_byte(std::size_t index,
                                                 unsigned char byte) {
  throw Error(std::format("invalid UTF-8 byte while serializing JSON string at "
                          "index {}: 0x{:02X}",
                          index, byte));
}

static bool try_append_json_ascii_byte(std::string &out, unsigned char c) {
  switch (c) {
  case '"':
    out += "\\\"";
    return true;
  case '\\':
    out += "\\\\";
    return true;
  case '\b':
    out += "\\b";
    return true;
  case '\f':
    out += "\\f";
    return true;
  case '\n':
    out += "\\n";
    return true;
  case '\r':
    out += "\\r";
    return true;
  case '\t':
    out += "\\t";
    return true;
  default:
    break;
  }

  if (c < 0x20) {
    append_json_control_char_escape(out, c);
    return true;
  }

  if (c < 0x80) {
    out.push_back(static_cast<char>(c));
    return true;
  }

  return false;
}

static bool is_utf8_continuation(unsigned char c) { return (c & 0xC0) == 0x80; }

static std::size_t utf8_sequence_size_from_lead(unsigned char lead) {
  if (lead >= 0xC2 && lead <= 0xDF) {
    return 2;
  }

  if (lead >= 0xE0 && lead <= 0xEF) {
    return 3;
  }

  if (lead >= 0xF0 && lead <= 0xF4) {
    return 4;
  }

  return 0;
}

static bool utf8_second_byte_in_lead_range(unsigned char lead,
                                           unsigned char second) {
  switch (lead) {
  case 0xE0:
    return second >= 0xA0;
  case 0xED:
    return second <= 0x9F;
  case 0xF0:
    return second >= 0x90;
  case 0xF4:
    return second <= 0x8F;
  default:
    return true;
  }
}

static std::size_t checked_utf8_sequence_size(const char *data, std::size_t n,
                                              std::size_t i) {
  // https://datatracker.ietf.org/doc/html/rfc3629#section-4
  unsigned char lead = static_cast<unsigned char>(data[i]);
  std::size_t size = utf8_sequence_size_from_lead(lead);

  if (size == 0 || i + size > n) {
    throw_invalid_utf8_byte(i, lead);
  }

  for (std::size_t offset = 1; offset < size; ++offset) {
    unsigned char c = static_cast<unsigned char>(data[i + offset]);
    if (!is_utf8_continuation(c)) {
      throw_invalid_utf8_byte(i + offset, c);
    }
  }

  if (!utf8_second_byte_in_lead_range(
          lead, static_cast<unsigned char>(data[i + 1]))) {
    throw_invalid_utf8_byte(i, lead);
  }

  return size;
}

static void append_bytes(std::string &out, std::string_view s) {
  // https://datatracker.ietf.org/doc/html/rfc8259#section-7
  const char *data = s.data();
  std::size_t n = s.size();
  std::size_t i = 0;
  while (i < n) {
    unsigned char c = static_cast<unsigned char>(data[i]);
    if (try_append_json_ascii_byte(out, c)) {
      ++i;
      continue;
    }

    std::size_t sequence_size = checked_utf8_sequence_size(data, n, i);
    out.append(data + i, sequence_size);
    i += sequence_size;
  }
}

static void append_bytes_string(std::string &out, std::string_view s) {
  out.push_back('"');
  append_bytes(out, s);
  out.push_back('"');
}

static void close_json_container(std::string &out, char close) {
  if (out.back() == ',') {
    out.back() = close;
  } else {
    out.push_back(close);
  }
}

static PathParts split_filter_path(std::string_view raw_path) {
  PathParts parts;

  std::size_t start = 0;
  while (start <= raw_path.size()) {
    std::size_t dot = raw_path.find('.', start);
    std::size_t end = dot == std::string_view::npos ? raw_path.size() : dot;
    std::string_view part = raw_path.substr(start, end - start);
    if (!part.empty()) {
      parts.emplace_back(part);
    }

    if (dot == std::string_view::npos) {
      break;
    }
    start = dot + 1;
  }

  return parts;
}

static PathFilters parse_filter_paths(EvalState &state, const PosIdx pos,
                                      Value &raw_paths) {
  state.forceList(raw_paths, pos, "while evaluating lazyToJSON filter paths");

  PathFilters filters;
  for (auto *elem : raw_paths.listView()) {
    if (elem == nullptr)
      continue;

    std::string_view filter = state.forceString(
        *elem, pos, "while evaluating a lazyToJSON filter path");
    auto parts = split_filter_path(filter);
    if (!parts.empty()) {
      filters.push_back(std::move(parts));
    }
  }

  return filters;
}

static bool path_contains_filter(const AttrPath &current_path,
                                 const PathParts &filter_path) {
  if (filter_path.empty() || filter_path.size() > current_path.size()) {
    return false;
  }

  const std::size_t max_start = current_path.size() - filter_path.size();
  for (std::size_t start = 0; start <= max_start; ++start) {
    bool all_match = true;
    for (std::size_t i = 0; i < filter_path.size(); ++i) {
      if (current_path[start + i] != filter_path[i]) {
        all_match = false;
        break;
      }
    }
    if (all_match) {
      return true;
    }
  }

  return false;
}

static bool should_skip_path(const AttrPath &current_path,
                             const PathFilters &filters) {
  for (const auto &filter_path : filters) {
    if (path_contains_filter(current_path, filter_path)) {
      return true;
    }
  }

  return false;
}

static std::string join_attr_path(const AttrPath &path) {
  std::string result;
  for (std::size_t i = 0; i < path.size(); ++i) {
    if (i != 0) {
      result.push_back('.');
    }
    result += path[i];
  }
  return result;
}

static bool append_seen_placeholder(SeenState &seen, const void *key,
                                    std::string &out) {
  if (auto active = seen.active.find(key); active != seen.active.end()) {
    out += "\"<recursive: ";
    append_bytes(out, active->second);
    out += ">\"";
  } else if (auto previous = seen.seen.find(key); previous != seen.seen.end()) {
    out += "\"<repeated: ";
    append_bytes(out, previous->second);
    out += ">\"";
  } else {
    return false;
  }
  return true;
}

static void enter_seen(SeenState &seen, const void *key,
                       const AttrPath &current_path) {
  std::string path;
  if (current_path.empty()) {
    path = "<root>";
  } else {
    path = join_attr_path(current_path);
  }

  seen.active.emplace(key, path);
  seen.seen.try_emplace(key, std::move(path));
}

static void append_value(EvalState &state, Value &v, std::string &out,
                         std::size_t depth, SeenState &seen,
                         const PathFilters &filters, AttrPath &current_path) {
  if (depth == 0) {
    out += "\"<max-depth>\"";
    return;
  }

  switch (v.type()) {
  case nThunk:
    if (v.isBlackhole()) {
      out += "\"<potential infinite recursion>\"";
    } else {
      out += "\"<thunk>\"";
    }
    return;

  case nFailed:
    out += "\"<failed>\"";
    return;

  case nInt:
    out += std::to_string(v.integer().value);
    return;

  case nFloat: {
    double d = v.fpoint();
    if (std::isnan(d)) {
      out += "\"<nan>\"";
      return;
    }
    if (!std::isfinite(d)) {
      out += d > 0 ? "\"<inf>\"" : "\"<-inf>\"";
      return;
    }
    std::format_to(std::back_inserter(out), "{:.17g}", d);
    return;
  }

  case nBool:
    out += v.boolean() ? "true" : "false";
    return;

  case nNull:
    out += "null";
    return;

  case nString:
    append_bytes_string(out, v.string_view());
    return;

  case nPath:
    append_bytes_string(out, v.path().to_string());
    return;

  case nAttrs: {
    if (state.isDerivation(v)) {
      out += "\"<derivation: ";
      auto *drvPath = v.attrs()->get(state.symbols.create("name"));
      if (drvPath == nullptr) {
        out += "null";
      } else if (drvPath->value->isThunk()) {
        out += "unforced name";
      } else if (drvPath->value->type() == nString) {
        append_bytes(out, drvPath->value->string_view());
      } else {
        out += "non-string name";
      }
      out += ">\"";
      return;
    }

    if (append_seen_placeholder(seen, &v, out)) {
      return;
    }

    enter_seen(seen, &v, current_path);

    out.push_back('{');
    for (auto *attr : v.attrs()->lexicographicOrder(state.symbols)) {
      std::string_view name = state.symbols[attr->name];
      append_bytes_string(out, name);
      out.push_back(':');

      if (attr->value == nullptr) {
        out += "\"<nullptr>\"";
      } else {
        current_path.push_back(name);
        if (should_skip_path(current_path, filters)) {
          out += "\"<skipped by lazy to json: ";
          append_bytes(out, join_attr_path(current_path));
          out += ">\"";
        } else {
          append_value(state, *attr->value, out, depth - 1, seen, filters,
                       current_path);
        }
        current_path.pop_back();
      }

      out.push_back(',');
    }
    seen.active.erase(&v);
    close_json_container(out, '}');
    return;
  }

  case nList: {
    if (append_seen_placeholder(seen, &v, out)) {
      return;
    }

    enter_seen(seen, &v, current_path);

    out.push_back('[');
    for (auto *elem : v.listView()) {
      if (elem == nullptr) {
        out += "\"<nullptr>\"";
      } else {
        append_value(state, *elem, out, depth - 1, seen, filters, current_path);
      }

      out.push_back(',');
    }
    seen.active.erase(&v);
    close_json_container(out, ']');
    return;
  }

  case nFunction:
    out += "\"<function>\"";
    return;

  case nExternal:
    out += "\"<external>\"";
    return;
  }

  out += "\"<unknown>\"";
}

static void prim_lazy_to_json(EvalState &state, const PosIdx pos, Value **args,
                              Value &v) {
  std::string json;
  SeenState seen;
  AttrPath current_path;
  PathFilters filters = parse_filter_paths(state, pos, *args[1]);
  // TODO force it for better experience?
  // state.forceValue(*args[0], pos);
  append_value(state, *args[0], json, kMaxDepth, seen, filters, current_path);
  v.mkString(json, state.mem);
}

static RegisterPrimOp primop_lazy_to_json(
    {.name = "__lazyToJSON",
     .args = {"value", "skipAttrPaths"},
     .doc = R"(Convert a Nix value to a JSON string without forcing thunks.)",
     .impl = prim_lazy_to_json});

} // namespace

extern "C" void nix_plugin_entry() {}
