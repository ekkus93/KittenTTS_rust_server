# Code Review TODO — Round 1

Items identified in the joint code review by Claude Sonnet 4.6 and GPT-5.4, March 2026.

Findings are grouped into three tiers: must-fix (correctness / safety), should-fix (risk
reduction), and optional polish (cleanup and clarity).

---

## Tier 1 — Must Fix (correctness / safety)

### 1.1 ORT shared-library version selection uses lexicographic sort, not semantic version order

**File:** `src/backend/kitten.rs` — `discover_default_ort_dylib_path` / `newest_ort_dylib_in_dir`

**Problem:**  
The auto-discovery path sorts `PathBuf` candidates with `right.cmp(left)` (reverse
lexicographic). This produces the wrong result for version names like `1.9.x` vs `1.10.x`:
the string `"1.9"` sorts *after* `"1.10"` because `'9' > '1'`, so a stale older release
wins over a newer one.

**Fix:**  
Parse the version directory name (the final path component) into a `(u32, u32, u32)` tuple
and use that as the sort key, falling back to lexicographic if the name is not parseable.

- [x] Fix version sort in `newest_ort_dylib_in_dir`
- [x] Fix version sort in `discover_default_ort_dylib_path` (candidate list)
- [x] Add a test covering the `1.9.x` vs `1.10.x` case

---

### 1.2 `env::set_var` called inside an active tokio runtime — unsafe in Rust 1.81+

**File:** `src/backend/kitten.rs` — `configure_default_ort_dylib_path` →
`OrtDylibSource::LocalDiscovery` branch

**Problem:**  
`std::env::set_var` is not thread-safe. Although it is called inside `spawn_blocking`, the
tokio thread pool is fully live at that point and other threads may concurrently call
`std::env::var` (e.g., inside ORT's own initialization or the dynamic linker). Rust 1.81
marks `set_var` as `unsafe` for this exact reason. The race can produce undefined behavior.

**Fix:**  
Resolve the library path before the tokio runtime starts and pass it through the
initialization chain explicitly, rather than writing it into the process environment as a
side effect. One clean approach: resolve the `OrtDylibSource` in `main` before
`#[tokio::main]` runs, convert `LocalDiscovery` to `Configured` by setting the env var
*before* the runtime starts, then proceed with normal init. Alternatively, store the
resolved path in `OrtRuntimeMetadata` and pass it into backend construction without relying
on env state.

- [x] Move ORT env-var mutation to pre-runtime startup (before `#[tokio::main]`)
- [x] Or: eliminate the `set_var` path entirely by passing the resolved path explicitly
- [x] Add a comment explaining the thread-safety constraint and chosen approach

---

## Tier 2 — Should Fix (risk reduction / improved clarity)

### 2.1 `channels: 0` placeholder in `supported_stream_format` is a silent footgun

**File:** `src/routes/tts.rs` — `supported_stream_format`

**Problem:**  
The function returns a `StreamFormat` with `channels: 0` as a deliberate placeholder,
relying on the sole call site to immediately overwrite it with a struct-update expression.
If `supported_stream_format` gains a second call site, or the update pattern is refactored
away, `channels: 0` will reach `validate_audio` and produce a confusing error.

**Fix:**  
Remove `channels` from `StreamFormat` entirely — it is always sourced from `settings.output_channels()`,
not from the format string. Pass it as a separate argument at construction time, or restructure
so `negotiate_stream_format` returns a `(container, sample_rate, media_type, header_value)`
tuple and the caller composes the final value. Either approach removes the placeholder.

- [ ] Remove `channels` field from `StreamFormat` or change it to `Option<u16>`
- [ ] Ensure `channels: 0` can no longer reach the audio pipeline

---

### 2.2 Poisoned-mutex recovery in request context should log a warning

**File:** `src/middleware/request_context.rs` — `with_context`

**Problem:**  
On a poisoned `Mutex`, the code recovers with `into_inner()` and continues silently. If a
handler panicked mid-write, subsequently logged metadata (voice, error code, text length)
may be inconsistent and there is no record of the anomaly.

**Fix:**  
Log `warn!` with the request ID when `lock()` returns `Err`. The recovery-and-continue
behavior is fine; the silence is the issue.

- [ ] Add `warn!` log on poisoned mutex in `with_context`
- [ ] Include the request ID in the warning where available

---

### 2.3 `output_channels()` wildcard arm should be `unreachable!()`, not a silent fallback

**File:** `src/config.rs` — `Settings::output_channels`

**Problem:**  
The `_` arm returns `1` silently. `validate()` already guarantees only `"mono"` or
`"stereo"` can reach this function, so the arm is dead code. If validation is ever
weakened or a new layout string is added, the silent `1` return would produce incorrect
audio without any diagnostic signal.

**Fix:**  
Replace `_ => 1` with `_ => unreachable!("channel_layout validated at config load time")`.

- [ ] Replace silent wildcard with `unreachable!()` in `output_channels`

---

## Tier 3 — Optional Polish

### 3.1 `HeaderValue::from_str(&bytes.len().to_string())` should use a typed conversion

**File:** `src/routes/tts.rs` — `build_binary_response`

`bytes.len().to_string()` always produces a valid ASCII decimal; `from_str` cannot fail.
The nominal error path is dead code and adds unnecessary visual noise.

- [ ] Replace with `HeaderValue::from(bytes.len())` (infallible `From<usize>`)

---

### 3.2 `const _:` signature-pinning pattern is unexplained

**Files:** `src/services/audio.rs`, `src/services/synth.rs`

The blocks of `const _: SomeFnType = some_fn;` enforce compile-time signature stability
for private functions, which is a legitimate technique. But without a comment, developers
unfamiliar with the pattern will waste time reverse-engineering the intent.

- [ ] Add a brief comment block before the first `const _:` group in each file explaining
  the intent (compile-time signature check for crate-private functions)

---

### 3.3 PCM quantization asymmetry should have a Python-compatibility comment

**File:** `src/services/audio.rs` — `float_audio_to_pcm`

`-1.0` maps to `-32767`, not `-32768`. This matches the Python server's `numpy * 32767`
behavior and is intentional. Without a comment, a future reviewer may "fix" this by
changing the multiplier to `32768.0`, breaking Python compatibility.

- [ ] Add a comment: `// Matches Python: multiply by 32767, not 32768, for symmetric range`

---

### 3.4 Config file-over-env precedence should be documented

**Files:** `src/config.rs` — `load_settings` docstring / README

**Background:**  
Both the Python server and the Rust port apply configuration in the order: defaults → env →
config file. This means a `settings.json` value wins over an environment variable with the
same name. This is the correct Python-compatible behavior, but it is the opposite of
12-factor app convention (where env typically wins). Without documentation this confuses
operators who expect Docker environment variables to override the config file.

Note: this was initially flagged as a bug in the code review. On inspection, it is correct
and intentional — the Rust port matches the Python server's documented precedence. The
action is purely documentation.

- [ ] Add a `load_settings` doc comment in `config.rs` stating the merge order explicitly
- [ ] Add a note to README explaining that the config file takes final precedence over env vars
  and why (Python-compatible design)

---

## Summary

| # | Tier | Item | File(s) |
|---|------|------|---------|
| 1.1 | Must Fix | ORT version sort is lexicographic, not semver | `backend/kitten.rs` |
| 1.2 | Must Fix | `env::set_var` inside tokio runtime — unsafe in Rust 1.81+ | `backend/kitten.rs` |
| 2.1 | Should Fix | `channels: 0` placeholder in stream format | `routes/tts.rs` |
| 2.2 | Should Fix | Poisoned mutex recovery should log a warning | `middleware/request_context.rs` |
| 2.3 | Should Fix | `output_channels()` wildcard should be `unreachable!()` | `config.rs` |
| 3.1 | Polish | `HeaderValue::from_str` on integer | `routes/tts.rs` |
| 3.2 | Polish | `const _:` pattern unexplained | `services/audio.rs`, `services/synth.rs` |
| 3.3 | Polish | PCM asymmetry needs Python-compatibility comment | `services/audio.rs` |
| 3.4 | Polish | Config file-over-env precedence needs documentation | `config.rs`, README |
