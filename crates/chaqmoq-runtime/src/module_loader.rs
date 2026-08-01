//! The module graph: ES modules from disk, over HTTPS, or inline as `data:`
//! URLs — with TypeScript and JSX transpiled on the fly.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_ast::SourceMapOption;
use deno_core::resolve_import;
use deno_core::ModuleLoadOptions;
use deno_core::ModuleLoadReferrer;
use deno_core::ModuleLoadResponse;
use deno_core::ModuleLoader;
use deno_core::ModuleResolveResponse;
use deno_core::ModuleSource;
use deno_core::ModuleSourceCode;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::RequestedModuleType;
use deno_core::ResolutionKind;
use deno_error::JsErrorBox;
use sha2::Digest as _;
use sha2::Sha256;

use crate::permissions::Permissions;

type SourceMapStore = Rc<RefCell<HashMap<String, Vec<u8>>>>;

/// Loads modules for a Chaqmoq program.
pub struct ChaqmoqModuleLoader {
    source_maps: SourceMapStore,
    permissions: Permissions,
    cache_dir: Option<PathBuf>,
    /// `true` disables the on-disk cache for remote modules (`--reload`).
    reload: bool,
}

impl ChaqmoqModuleLoader {
    pub fn new(permissions: Permissions, reload: bool) -> Self {
        let cache_dir = dirs::cache_dir().map(|dir| dir.join("chaqmoq").join("remote"));
        Self {
            source_maps: Rc::new(RefCell::new(HashMap::new())),
            permissions,
            cache_dir,
            reload,
        }
    }

    /// Where the module graph stashes downloaded dependencies.
    pub fn cache_dir(&self) -> Option<&PathBuf> {
        self.cache_dir.as_ref()
    }
}

/// Maps a media type onto "what V8 should be handed" plus "does it need
/// transpiling first".
fn classify(media_type: MediaType) -> Result<(ModuleType, bool), JsErrorBox> {
    Ok(match media_type {
        MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => (ModuleType::JavaScript, false),
        MediaType::Jsx => (ModuleType::JavaScript, true),
        MediaType::TypeScript
        | MediaType::Mts
        | MediaType::Cts
        | MediaType::Dts
        | MediaType::Dmts
        | MediaType::Dcts
        | MediaType::Tsx => (ModuleType::JavaScript, true),
        MediaType::Json => (ModuleType::Json, false),
        MediaType::Wasm => (ModuleType::Wasm, false),
        other => {
            return Err(JsErrorBox::type_error(format!(
                "Unsupported module type: {other}"
            )))
        }
    })
}

/// Strips types (and compiles JSX) with swc via `deno_ast`.
///
/// This is transpile-only: Chaqmoq does not type-check, exactly like
/// `deno run --no-check` or `tsx`. Type errors surface in your editor or CI,
/// not at runtime.
fn transpile(
    specifier: &ModuleSpecifier,
    media_type: MediaType,
    code: String,
    source_maps: &SourceMapStore,
) -> Result<String, JsErrorBox> {
    let parsed = deno_ast::parse_module(ParseParams {
        specifier: specifier.clone(),
        text: code.into(),
        media_type,
        capture_tokens: false,
        scope_analysis: false,
        maybe_syntax: None,
    })
    .map_err(JsErrorBox::from_err)?;

    let transpiled = parsed
        .transpile(
            &deno_ast::TranspileOptions {
                imports_not_used_as_values: deno_ast::ImportsNotUsedAsValues::Remove,
                decorators: deno_ast::DecoratorsTranspileOption::Ecma,
                ..Default::default()
            },
            &deno_ast::TranspileModuleOptions { module_kind: None },
            &deno_ast::EmitOptions {
                source_map: SourceMapOption::Separate,
                inline_sources: true,
                ..Default::default()
            },
        )
        .map_err(JsErrorBox::from_err)?
        .into_source();

    // Registering the source map is what turns a stack frame pointing at
    // generated JS back into the original `.ts` line and column.
    if let Some(map) = transpiled.source_map {
        source_maps
            .borrow_mut()
            .insert(specifier.to_string(), map.into_bytes());
    }

    Ok(transpiled.text)
}

/// Strips TypeScript types from a standalone snippet.
///
/// `chaqmoq eval --ts` and similar callers hand code straight to V8 without
/// going through the module loader, so they need this entry point.
pub fn transpile_snippet(specifier: &ModuleSpecifier, code: String) -> Result<String, JsErrorBox> {
    let media_type = MediaType::from_specifier(specifier);
    let store: SourceMapStore = Rc::new(RefCell::new(HashMap::new()));
    transpile(specifier, media_type, code, &store)
}

/// Turns raw bytes into a `ModuleSource`, transpiling when needed.
fn build_source(
    specifier: &ModuleSpecifier,
    code: String,
    media_type: MediaType,
    requested: &RequestedModuleType,
    source_maps: &SourceMapStore,
) -> Result<ModuleSource, JsErrorBox> {
    let (module_type, should_transpile) = classify(media_type)?;

    // `import data from "./x.json" with { type: "json" }` must not be evaluated
    // as JavaScript even if the extension lies.
    let module_type = match requested {
        RequestedModuleType::Json => ModuleType::Json,
        _ => module_type,
    };

    let code = if should_transpile && module_type == ModuleType::JavaScript {
        transpile(specifier, media_type, code, source_maps)?
    } else {
        code
    };

    Ok(ModuleSource::new(
        module_type,
        ModuleSourceCode::String(code.into()),
        specifier,
        None,
    ))
}

fn cache_path(cache_dir: &std::path::Path, specifier: &ModuleSpecifier) -> PathBuf {
    let digest = Sha256::digest(specifier.as_str().as_bytes());
    let name = format!("{digest:x}");
    // Keep the extension so `MediaType::from_specifier` still works on reload
    // and so the cache is browsable by hand.
    let extension = std::path::Path::new(specifier.path())
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("js");
    cache_dir.join(format!("{name}.{extension}"))
}

impl ModuleLoader for ChaqmoqModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> ModuleResolveResponse {
        // Give a targeted error for bare specifiers rather than the generic
        // "relative import path not prefixed with /" from the URL parser.
        let resolved = resolve_import(specifier, referrer).map_err(|err| {
            if !specifier.starts_with('.')
                && !specifier.starts_with('/')
                && !specifier.contains(':')
            {
                JsErrorBox::type_error(format!(
                    "Relative import path \"{specifier}\" not prefixed with / or ./ or ../, \
                     and Chaqmoq has no package manager. Import a URL or a local file instead."
                ))
            } else {
                JsErrorBox::from_err(err)
            }
        })?;
        Ok(resolved)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleLoadReferrer>,
        options: ModuleLoadOptions,
    ) -> ModuleLoadResponse {
        let specifier = module_specifier.clone();
        let source_maps = self.source_maps.clone();
        let requested = options.requested_module_type.clone();

        match specifier.scheme() {
            "file" => {
                let result = (|| {
                    let path = specifier.to_file_path().map_err(|_| {
                        JsErrorBox::type_error(format!("Invalid file URL: {specifier}"))
                    })?;
                    // Local modules are *code*, not data: running `chaqmoq run
                    // app.ts` is itself the grant. `--allow-read` gates
                    // `Chaqmoq.readTextFile` and friends, which is where a
                    // program could actually exfiltrate file contents.
                    let code = std::fs::read_to_string(&path)
                        .map_err(|e| crate::ops::io_error(e, &path.display().to_string()))?;
                    let media_type = MediaType::from_specifier(&specifier);
                    build_source(&specifier, code, media_type, &requested, &source_maps)
                })();
                ModuleLoadResponse::Sync(result)
            }

            "http" | "https" => {
                let permissions = self.permissions.clone();
                let cache_dir = self.cache_dir.clone();
                let reload = self.reload;

                ModuleLoadResponse::Async(Box::pin(async move {
                    // Remote modules do reach the network, so they stay behind
                    // `--allow-net` even though local ones are ungated.
                    permissions.check_net_url(&specifier)?;

                    let cached = cache_dir.as_ref().map(|dir| cache_path(dir, &specifier));

                    let code = match &cached {
                        Some(path) if !reload && path.exists() => std::fs::read_to_string(path)
                            .map_err(|e| crate::ops::io_error(e, &path.display().to_string()))?,
                        _ => {
                            let response = reqwest::get(specifier.clone()).await.map_err(|e| {
                                JsErrorBox::generic(format!(
                                    "failed to fetch module {specifier}: {e}"
                                ))
                            })?;
                            if !response.status().is_success() {
                                return Err(JsErrorBox::generic(format!(
                                    "module not found: {specifier} (HTTP {})",
                                    response.status()
                                )));
                            }
                            let text = response.text().await.map_err(|e| {
                                JsErrorBox::generic(format!(
                                    "failed to read module {specifier}: {e}"
                                ))
                            })?;
                            // A cache write failure must not break the program.
                            if let Some(path) = &cached {
                                if let Some(parent) = path.parent() {
                                    let _ = std::fs::create_dir_all(parent);
                                }
                                let _ = std::fs::write(path, &text);
                            }
                            text
                        }
                    };

                    let media_type = MediaType::from_specifier(&specifier);
                    let source =
                        build_source(&specifier, code, media_type, &requested, &source_maps)?;
                    Ok(source)
                }))
            }

            "data" => {
                let result = load_data_url(&specifier, &requested, &source_maps);
                ModuleLoadResponse::Sync(result)
            }

            scheme => ModuleLoadResponse::Sync(Err(JsErrorBox::type_error(format!(
                "Unsupported module scheme \"{scheme}:\" in {specifier}. \
                 Chaqmoq supports file:, http:, https: and data: URLs."
            )))),
        }
    }

    fn get_source_map(&self, specifier: &str) -> Option<Cow<'_, [u8]>> {
        self.source_maps
            .borrow()
            .get(specifier)
            .map(|bytes| Cow::Owned(bytes.clone()))
    }
}

fn load_data_url(
    specifier: &ModuleSpecifier,
    requested: &RequestedModuleType,
    source_maps: &SourceMapStore,
) -> Result<ModuleSource, JsErrorBox> {
    let raw = specifier.path();
    let (meta, payload) = raw
        .split_once(',')
        .ok_or_else(|| JsErrorBox::type_error(format!("Invalid data URL: {specifier}")))?;

    let code = if meta.ends_with(";base64") {
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(payload)
            .map_err(|e| JsErrorBox::type_error(format!("Invalid base64 data URL: {e}")))?;
        String::from_utf8(bytes)
            .map_err(|e| JsErrorBox::type_error(format!("data URL is not valid UTF-8: {e}")))?
    } else {
        percent_decode(payload)
    };

    let mime = meta.split(';').next().unwrap_or("text/javascript");
    let media_type = match mime {
        "application/json" => MediaType::Json,
        "text/typescript" | "application/typescript" | "video/mp2t" => MediaType::TypeScript,
        "text/tsx" => MediaType::Tsx,
        "text/jsx" => MediaType::Jsx,
        _ => MediaType::JavaScript,
    };

    build_source(specifier, code, media_type, requested, source_maps)
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_typescript_as_javascript_needing_transpile() {
        let (module_type, transpile) = classify(MediaType::TypeScript).unwrap();
        assert_eq!(module_type, ModuleType::JavaScript);
        assert!(transpile);
    }

    #[test]
    fn classifies_json_without_transpile() {
        let (module_type, transpile) = classify(MediaType::Json).unwrap();
        assert_eq!(module_type, ModuleType::Json);
        assert!(!transpile);
    }

    #[test]
    fn cache_path_is_stable_and_keeps_extension() {
        let dir = std::path::Path::new("/cache");
        let spec = ModuleSpecifier::parse("https://example.com/mod.ts").unwrap();
        let first = cache_path(dir, &spec);
        let second = cache_path(dir, &spec);
        assert_eq!(first, second);
        assert_eq!(first.extension().unwrap(), "ts");
    }

    #[test]
    fn percent_decoding_handles_escapes() {
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("plain"), "plain");
    }
}
