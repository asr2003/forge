use std::collections::HashSet;
use std::path::Path;

use base64::Engine;
use forge_domain::Attachment;
use futures::TryFutureExt;
use lazy_static::lazy_static;

// TODO: bring pdf support, pdf is just a collection of images.

lazy_static! {
    static ref IMAGE_TYPES: HashSet<&'static str> = {
        let mut set = HashSet::new();
        set.insert("jpg");
        set.insert("jpeg");
        set.insert("png");
        set.insert("gif");
        set.insert("webp");
        set
    };
}

pub async fn prepare_attachments<T: AsRef<Path>>(paths: Vec<T>) -> HashSet<Attachment> {
    futures::future::join_all(
        paths
            .into_iter()
            .map(|v| v.as_ref().to_path_buf())
            .filter(|v| v.extension().is_some())
            .filter(|v| IMAGE_TYPES.contains(v.extension().unwrap().to_string_lossy().as_ref()))
            .map(|v| {
                let ext = v.extension().unwrap().to_string_lossy().to_string();
                tokio::fs::read(v).map_ok(|v| {
                    format!(
                        "data:image/{};base64,{}",
                        ext.strip_prefix('.').map(String::from).unwrap_or(ext),
                        base64::engine::general_purpose::STANDARD.encode(v)
                    )
                })
            }),
    )
    .await
    .into_iter()
    .filter_map(|v| v.ok())
    .map(|v| Attachment { data: v })
    .collect::<HashSet<_>>()
}

pub async fn split_image_paths<T: ToString>(v: T) -> (String, HashSet<Attachment>) {
    let chat = v.to_string();
    let words = chat.split(" ").map(|v| v.to_string()).collect::<Vec<_>>();

    (chat, prepare_attachments(words).await)
}
