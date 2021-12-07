#[derive(Debug, serde::Deserialize)]
pub struct Config {
    pub name: String,
    pub package_key: Option<String>,
    pub repo_key: Option<String>,
    pub srcdest: String,
    pub logdest: String,
    pub pkgbuild: String,
    pub builds: std::collections::HashMap<super::builder::Arch, BuildConfig>,
    pub s3: Option<S3Config>,
}

#[derive(Debug, serde::Deserialize)]
pub struct BuildConfig {
    pub chroot: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct S3Config {
    pub bucket: String,
    pub region: String,
}

impl Config {
    pub fn from_reader<R>(reader: R) -> serde_yaml::Result<Self>
    where
        R: std::io::Read,
    {
        serde_yaml::from_reader(reader)
    }

    pub fn repo_dir(&self, arch: &super::builder::Arch) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.name)
            .join("os")
            .join(format!("{}", arch))
    }

    pub fn db_path(&self, arch: &super::builder::Arch) -> std::path::PathBuf {
        let mut path = self.repo_dir(arch).join(&self.name).into_os_string();
        path.push(".db");
        std::path::PathBuf::from(path)
    }

    pub fn files_path(&self, arch: &super::builder::Arch) -> std::path::PathBuf {
        let mut path = self.repo_dir(arch).join(&self.name).into_os_string();
        path.push(".files");
        std::path::PathBuf::from(path)
    }

    pub fn package_dir(&self, package_name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.pkgbuild).join(package_name)
    }
}

pub struct S3 {
    client: aws_sdk_s3::Client,
    bucket: String,
}

impl S3 {
    pub async fn new(config: S3Config) -> Self {
        let shared_config = aws_config::load_from_env().await;
        let conf = aws_sdk_s3::config::Builder::from(&shared_config)
            .region(Some(aws_sdk_s3::Region::new(config.region)))
            .build();
        let client = aws_sdk_s3::Client::from_conf(conf);
        S3 {
            client,
            bucket: config.bucket,
        }
    }

    pub async fn download_repository(
        &self,
        config: &Config,
        arch: &super::builder::Arch,
    ) -> Result<(), anyhow::Error> {
        let (r1, r2) = futures::join!(
            self.get(config.db_path(arch)),
            self.get(config.files_path(arch))
        );
        r1?;
        r2
    }

    pub async fn upload_repository<P>(
        &self,
        config: &Config,
        arch: &super::builder::Arch,
        package_paths: &[P],
    ) -> Result<(), anyhow::Error>
    where
        P: AsRef<std::path::Path>,
    {
        const XZ_MIME_TYPE: &str = "application/x-xz";
        const ZST_MIME_TYPE: &str = "application/zstd";
        const OCTET_STREAM_MIME_TYPE: &str = "application/octet-stream";
        const SIG_MIME_TYPE: &str = "application/pgp-signature";
        const GZIP_MIME_TYPE: &str = "application/gzip";

        let mut futures_unordered = futures::stream::FuturesUnordered::new();
        for package_path in package_paths {
            let mime_type = if let Some(ext) = package_path.as_ref().extension() {
                if ext == "zst" {
                    ZST_MIME_TYPE
                } else if ext == "xz" {
                    XZ_MIME_TYPE
                } else {
                    OCTET_STREAM_MIME_TYPE
                }
            } else {
                OCTET_STREAM_MIME_TYPE
            };
            futures_unordered.push(self.put(package_path.as_ref().to_owned(), mime_type));
            if config.package_key.is_some() {
                let mut sig_path = package_path.as_ref().as_os_str().to_os_string();
                sig_path.push(".sig");
                futures_unordered.push(self.put(std::path::PathBuf::from(sig_path), SIG_MIME_TYPE));
            }
        }
        futures_unordered.push(self.put(config.files_path(arch), GZIP_MIME_TYPE));
        let db_path = config.db_path(arch);
        futures_unordered.push(self.put(db_path.to_owned(), GZIP_MIME_TYPE));
        if config.repo_key.is_some() {
            let mut sig_path = db_path.into_os_string();
            sig_path.push(".sig");
            futures_unordered.push(self.put(std::path::PathBuf::from(sig_path), SIG_MIME_TYPE));
        }
        use futures::StreamExt as _;
        while let Some(result) = futures_unordered.next().await {
            result?;
        }
        Ok(())
    }

    async fn get<P>(&self, path: P) -> Result<(), anyhow::Error>
    where
        P: AsRef<std::path::Path>,
    {
        let path = path.as_ref();
        println!("Download {}", path.display());
        match self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(path.to_string_lossy())
            .send()
            .await
        {
            Ok(mut output) => {
                use futures::StreamExt as _;
                use tokio::io::AsyncWriteExt as _;

                let file = tokio::fs::File::create(path).await?;
                let mut writer = tokio::io::BufWriter::new(file);
                while let Some(item) = output.body.next().await {
                    writer.write_all(&item?).await?;
                }
                writer.shutdown().await?;
                Ok(())
            }
            Err(aws_sdk_s3::SdkError::ServiceError {
                err:
                    aws_sdk_s3::error::GetObjectError {
                        kind: aws_sdk_s3::error::GetObjectErrorKind::NoSuchKey(_),
                        ..
                    },
                ..
            }) => Ok(()),
            Err(e) => Err(anyhow::Error::from(e)),
        }
    }

    async fn put<P>(&self, path: P, content_type: &str) -> Result<(), anyhow::Error>
    where
        P: AsRef<std::path::Path>,
    {
        let path = path.as_ref();
        let metadata = tokio::fs::metadata(path).await?;
        let stream = aws_sdk_s3::ByteStream::from_path(path).await?;
        let request = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(path.to_string_lossy())
            .content_type(content_type)
            .content_length(metadata.len() as i64)
            .body(stream);
        println!("Upload {}", path.display());
        request.send().await?;
        Ok(())
    }
}
