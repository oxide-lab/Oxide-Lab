//! Adapted from Jan's tauri-plugin-llamacpp argument builder (Apache-2.0).
//! Source reference: example/clients/jan/src-tauri/plugins/tauri-plugin-llamacpp/src/args.rs

use crate::core::types::{LlamaRuntimeConfig, LlamaSessionKind};

pub struct LlamaArgumentBuilder {
    cfg: LlamaRuntimeConfig,
    kind: LlamaSessionKind,
}

impl LlamaArgumentBuilder {
    pub fn new(cfg: LlamaRuntimeConfig, kind: LlamaSessionKind) -> Self {
        Self { cfg, kind }
    }

    pub fn build(self, model_id: &str, model_path: &str, port: u16) -> Vec<String> {
        let mut args = vec![
            "--no-webui".to_string(),
            "--jinja".to_string(),
            "-m".to_string(),
            model_path.to_string(),
            "-a".to_string(),
            model_id.to_string(),
            "--port".to_string(),
            port.to_string(),
            // Core perf/runtime controls.
            "-ngl".to_string(),
            self.cfg.n_gpu_layers.max(0).to_string(),
        ];

        if self.cfg.threads > 0 {
            args.push("--threads".to_string());
            args.push(self.cfg.threads.to_string());
        }
        if self.cfg.threads_batch > 0 {
            args.push("--threads-batch".to_string());
            args.push(self.cfg.threads_batch.to_string());
        }
        if self.cfg.batch_size > 0 {
            args.push("--batch-size".to_string());
            args.push(self.cfg.batch_size.to_string());
        }
        if self.cfg.ubatch_size > 0 {
            args.push("--ubatch-size".to_string());
            args.push(self.cfg.ubatch_size.to_string());
        }

        if self.cfg.flash_attn != "auto" && !self.cfg.flash_attn.is_empty() {
            args.push("--flash-attn".to_string());
            args.push(self.cfg.flash_attn.clone());
        }

        match self.kind {
            LlamaSessionKind::Embedding => {
                args.push("--embedding".to_string());
                args.push("--pooling".to_string());
                args.push("mean".to_string());
            }
            LlamaSessionKind::Chat => {
                if self.cfg.ctx_size > 0 {
                    args.push("--ctx-size".to_string());
                    args.push(self.cfg.ctx_size.to_string());
                }
                if self.cfg.n_predict != 0 {
                    args.push("--n-predict".to_string());
                    args.push(self.cfg.n_predict.to_string());
                }
            }
        }

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_embedding_args() {
        let cfg = LlamaRuntimeConfig::default();
        let args = LlamaArgumentBuilder::new(cfg, LlamaSessionKind::Embedding).build("m", "x", 1);
        assert!(args.contains(&"--embedding".to_string()));
        assert!(args.contains(&"--pooling".to_string()));
    }
}
