use crate::{
    error::{Error, UploadError},
    magick::ValidInputType,
    migrate::{alias_id_key, alias_key},
    store::Store,
    upload_manager::{
        delete_key,
        hasher::{Hash, Hasher},
        UploadManager,
    },
};
use actix_web::web;
use futures_util::stream::{Stream, StreamExt};
use tracing::{debug, instrument, warn, Span};
use tracing_futures::Instrument;
use uuid::Uuid;

pub(crate) struct UploadManagerSession<S: Store + Clone + 'static>
where
    Error: From<S::Error>,
{
    store: S,
    manager: UploadManager,
    alias: Option<String>,
    finished: bool,
}

impl<S: Store + Clone + 'static> UploadManagerSession<S>
where
    Error: From<S::Error>,
{
    pub(super) fn new(manager: UploadManager, store: S) -> Self {
        UploadManagerSession {
            store,
            manager,
            alias: None,
            finished: false,
        }
    }

    pub(crate) fn succeed(mut self) {
        self.finished = true;
    }

    pub(crate) fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }
}

enum Dup {
    Exists,
    New,
}

impl Dup {
    fn exists(&self) -> bool {
        matches!(self, Dup::Exists)
    }
}

impl<S: Store + Clone + 'static> Drop for UploadManagerSession<S>
where
    Error: From<S::Error>,
{
    fn drop(&mut self) {
        if self.finished {
            return;
        }

        if let Some(alias) = self.alias.take() {
            let store = self.store.clone();
            let manager = self.manager.clone();
            let cleanup_span = tracing::info_span!(
                parent: None,
                "Upload cleanup",
                alias = &tracing::field::display(&alias),
            );
            cleanup_span.follows_from(Span::current());
            actix_rt::spawn(
                async move {
                    // undo alias -> hash mapping
                    debug!("Remove alias -> hash mapping");
                    if let Ok(Some(hash)) = manager.inner.alias_tree.remove(&alias) {
                        // undo alias -> id mapping
                        debug!("Remove alias -> id mapping");
                        let key = alias_id_key(&alias);
                        if let Ok(Some(id)) = manager.inner.alias_tree.remove(&key) {
                            // undo hash/id -> alias mapping
                            debug!("Remove hash/id -> alias mapping");
                            let id = String::from_utf8_lossy(&id);
                            let key = alias_key(&hash, &id);
                            let _ = manager.inner.main_tree.remove(key);
                        }

                        let _ = manager.check_delete_files(store, hash).await;
                    }
                }
                .instrument(cleanup_span),
            );
        }
    }
}

impl<S: Store> UploadManagerSession<S>
where
    Error: From<S::Error>,
{
    /// Generate a delete token for an alias
    #[instrument(skip(self))]
    pub(crate) async fn delete_token(&self) -> Result<String, Error> {
        let alias = self.alias.clone().ok_or(UploadError::MissingAlias)?;

        debug!("Generating delete token");
        let s: String = Uuid::new_v4().to_string();
        let delete_token = s.clone();

        debug!("Saving delete token");
        let alias_tree = self.manager.inner.alias_tree.clone();
        let key = delete_key(&alias);
        let res = web::block(move || {
            alias_tree.compare_and_swap(
                key.as_bytes(),
                None as Option<sled::IVec>,
                Some(s.as_bytes()),
            )
        })
        .await??;

        if let Err(sled::CompareAndSwapError {
            current: Some(ivec),
            ..
        }) = res
        {
            let s = String::from_utf8(ivec.to_vec())?;

            debug!("Returning existing delete token, {}", s);
            return Ok(s);
        }

        debug!("Returning new delete token, {}", delete_token);
        Ok(delete_token)
    }

    /// Upload the file while preserving the filename, optionally validating the uploaded image
    #[instrument(skip(self, stream))]
    pub(crate) async fn import(
        mut self,
        alias: String,
        validate: bool,
        mut stream: impl Stream<Item = Result<web::Bytes, Error>> + Unpin,
    ) -> Result<Self, Error> {
        let mut bytes_mut = actix_web::web::BytesMut::new();

        debug!("Reading stream to memory");
        while let Some(res) = stream.next().await {
            let bytes = res?;
            bytes_mut.extend_from_slice(&bytes);
        }

        debug!("Validating bytes");
        let (content_type, validated_reader) = crate::validate::validate_image_bytes(
            bytes_mut.freeze(),
            self.manager.inner.format,
            validate,
        )
        .await?;

        let mut hasher_reader = Hasher::new(validated_reader, self.manager.inner.hasher.clone());

        let filename = self.next_file(content_type).await?;

        let identifier = self
            .store
            .save_async_read(&mut hasher_reader, &filename)
            .await?;
        let hash = hasher_reader.finalize_reset().await?;

        debug!("Storing alias");
        self.alias = Some(alias.clone());
        self.add_existing_alias(&hash, &alias).await?;

        debug!("Saving file");
        self.save_upload(&identifier, hash, filename).await?;

        // Return alias to file
        Ok(self)
    }

    /// Upload the file, discarding bytes if it's already present, or saving if it's new
    #[instrument(skip(self, stream))]
    pub(crate) async fn upload(
        mut self,
        mut stream: impl Stream<Item = Result<web::Bytes, Error>> + Unpin,
    ) -> Result<Self, Error> {
        let mut bytes_mut = actix_web::web::BytesMut::new();

        debug!("Reading stream to memory");
        while let Some(res) = stream.next().await {
            let bytes = res?;
            bytes_mut.extend_from_slice(&bytes);
        }

        debug!("Validating bytes");
        let (input_type, validated_reader) = crate::validate::validate_image_bytes(
            bytes_mut.freeze(),
            self.manager.inner.format,
            true,
        )
        .await?;

        let mut hasher_reader = Hasher::new(validated_reader, self.manager.inner.hasher.clone());

        let filename = self.next_file(input_type).await?;

        let identifier = self
            .store
            .save_async_read(&mut hasher_reader, &filename)
            .await?;
        let hash = hasher_reader.finalize_reset().await?;

        debug!("Adding alias");
        self.add_alias(&hash, input_type).await?;

        debug!("Saving file");
        self.save_upload(&identifier, hash, filename).await?;

        // Return alias to file
        Ok(self)
    }

    // check duplicates & store image if new
    async fn save_upload(
        &self,
        identifier: &S::Identifier,
        hash: Hash,
        filename: String,
    ) -> Result<(), Error> {
        let dup = self.check_duplicate(hash, filename.clone()).await?;

        // bail early with alias to existing file if this is a duplicate
        if dup.exists() {
            debug!("Duplicate exists, removing file");

            self.store.remove(identifier).await?;
            return Ok(());
        }

        self.manager.store_identifier(filename, identifier).await?;

        Ok(())
    }

    // check for an already-uploaded image with this hash, returning the path to the target file
    #[instrument(skip(self, hash))]
    async fn check_duplicate(&self, hash: Hash, filename: String) -> Result<Dup, Error> {
        let main_tree = self.manager.inner.main_tree.clone();

        let filename2 = filename.clone();
        let hash2 = hash.as_slice().to_vec();
        debug!("Inserting filename for hash");
        let res = web::block(move || {
            main_tree.compare_and_swap(
                hash2,
                None as Option<sled::IVec>,
                Some(filename2.as_bytes()),
            )
        })
        .await??;

        if let Err(sled::CompareAndSwapError {
            current: Some(ivec),
            ..
        }) = res
        {
            let name = String::from_utf8(ivec.to_vec())?;
            debug!("Filename exists for hash, {}", name);
            return Ok(Dup::Exists);
        }

        let fname_tree = self.manager.inner.filename_tree.clone();
        debug!("Saving filename -> hash relation");
        web::block(move || fname_tree.insert(filename, hash.into_inner())).await??;

        Ok(Dup::New)
    }

    // generate a short filename that isn't already in-use
    #[instrument(skip(self, input_type))]
    async fn next_file(&self, input_type: ValidInputType) -> Result<String, Error> {
        loop {
            debug!("Filename generation loop");
            let filename = file_name(Uuid::new_v4(), input_type);

            let identifier_tree = self.manager.inner.identifier_tree.clone();
            let filename2 = filename.clone();
            let filename_exists = web::block(move || identifier_tree.get(filename2.as_bytes()))
                .await??
                .is_some();

            if !filename_exists {
                return Ok(filename);
            }

            debug!("Filename exists, trying again");
        }
    }

    #[instrument(skip(self, hash, alias))]
    async fn add_existing_alias(&self, hash: &Hash, alias: &str) -> Result<(), Error> {
        self.save_alias_hash_mapping(hash, alias).await??;

        self.store_hash_id_alias_mapping(hash, alias).await?;

        Ok(())
    }

    // Add an alias to an existing file
    //
    // This will help if multiple 'users' upload the same file, and one of them wants to delete it
    #[instrument(skip(self, hash, input_type))]
    async fn add_alias(&mut self, hash: &Hash, input_type: ValidInputType) -> Result<(), Error> {
        let alias = self.next_alias(hash, input_type).await?;

        self.store_hash_id_alias_mapping(hash, &alias).await?;

        Ok(())
    }

    // Add a pre-defined alias to an existin file
    //
    // DANGER: this can cause BAD BAD BAD conflicts if the same alias is used for multiple files
    #[instrument(skip(self, hash))]
    async fn store_hash_id_alias_mapping(&self, hash: &Hash, alias: &str) -> Result<(), Error> {
        let alias = alias.to_string();
        loop {
            debug!("hash -> alias save loop");
            let db = self.manager.inner.db.clone();
            let id = web::block(move || db.generate_id()).await??.to_string();

            let alias_tree = self.manager.inner.alias_tree.clone();
            let key = alias_id_key(&alias);
            let id2 = id.clone();
            debug!("Saving alias -> id mapping");
            web::block(move || alias_tree.insert(key.as_bytes(), id2.as_bytes())).await??;

            let key = alias_key(hash.as_slice(), &id);
            let main_tree = self.manager.inner.main_tree.clone();
            let alias2 = alias.clone();
            debug!("Saving hash/id -> alias mapping");
            let res = web::block(move || {
                main_tree.compare_and_swap(key, None as Option<sled::IVec>, Some(alias2.as_bytes()))
            })
            .await??;

            if res.is_ok() {
                break;
            }

            debug!("Id exists, trying again");
        }

        Ok(())
    }

    // Generate an alias to the file
    #[instrument(skip(self, hash, input_type))]
    async fn next_alias(
        &mut self,
        hash: &Hash,
        input_type: ValidInputType,
    ) -> Result<String, Error> {
        loop {
            debug!("Alias gen loop");
            let alias = file_name(Uuid::new_v4(), input_type);
            self.alias = Some(alias.clone());

            let res = self.save_alias_hash_mapping(hash, &alias).await?;

            if res.is_ok() {
                return Ok(alias);
            }
            debug!("Alias exists, regenning");
        }
    }

    // Save an alias to the database
    #[instrument(skip(self, hash))]
    async fn save_alias_hash_mapping(
        &self,
        hash: &Hash,
        alias: &str,
    ) -> Result<Result<(), Error>, Error> {
        let tree = self.manager.inner.alias_tree.clone();
        let vec = hash.as_slice().to_vec();
        let alias = alias.to_string();

        debug!("Saving alias -> hash mapping");
        let res = web::block(move || {
            tree.compare_and_swap(alias.as_bytes(), None as Option<sled::IVec>, Some(vec))
        })
        .await??;

        if res.is_err() {
            warn!("Duplicate alias");
            return Ok(Err(UploadError::DuplicateAlias.into()));
        }

        Ok(Ok(()))
    }
}

fn file_name(name: Uuid, input_type: ValidInputType) -> String {
    format!("{}{}", name, input_type.as_ext())
}
