mod db;
mod error;
mod thumbnail;
mod utils;

use lazy_static::lazy_static;
use sha2::{Digest, Sha224};
use std::{
    collections::{HashMap, VecDeque},
    fs, io,
    path::Path,
    path::PathBuf,
};

use db::DB;

pub use db::Item;
pub use error::{Error, ErrorKind, Result};

lazy_static! {
    /// Maps from supported MIME types from their default extension
    static ref SUPPORTED_MIMETYPES: HashMap<&'static str, &'static str> = {
        let mut supported_mimetypes = HashMap::new();
        supported_mimetypes.insert("video/mp4", "mp4");
        supported_mimetypes
    };
}

pub struct Repo {
    db: DB,
    path: PathBuf,
    magic_cookie: magic::Cookie,
}

impl Repo {
    /// Creates or opens a vorg repo.
    ///
    /// If the provided path does not exist, creates a new vorg repo.
    /// If the provided path exists, it performs basic checks to make sure the repo is valid.
    /// For more thorough checks on repo integrity, see `check_data_integrity`.
    ///
    /// # Errors
    ///
    /// - `ErrorKind::IO` if repo does not exist (determined by existence of vorg.db) and vorg
    ///   encountered IO errors when trying to create one, e.g. permission denied, folder creation
    ///   failed, etc.
    /// - `ErrorKind::StoreFolder` or `ErrorKind::ThumbnailFolder` if repo exists and has invalid
    ///   file store or thumbnail store.
    /// - `ErrorKind::DB` if database (vorg.db) exists and is invalid. Or database does not exist
    ///   and failed to be created.
    /// - `ErrorKind::Magic` if failed to initialize libmagic.
    pub async fn new<T>(path: T) -> Result<Self>
    where
        T: AsRef<Path>,
    {
        let path = path.as_ref();

        // Attempt to create the repo folder
        fs::create_dir_all(path)?;
        if path.join("vorg.db").is_file() {
            // Repo exists, validate it
            Repo::validate_repo(path).await
        } else {
            // Repo doesn't exist, create it
            Repo::create_repo(path).await
        }
    }

    async fn create_repo<T>(repo_path: T) -> Result<Self>
    where
        T: AsRef<Path>,
    {
        let repo_path = repo_path.as_ref();

        // Create store
        let store_path = repo_path.join("store");
        fs::create_dir_all(&store_path)?;

        // Create thumbnail store
        let thumbnail_path = repo_path.join("thumbnail");
        fs::create_dir_all(&thumbnail_path)?;

        // Create DB
        Ok(Repo {
            path: repo_path.to_owned(),
            db: DB::new(repo_path.join("vorg.db")).await?,
            magic_cookie: Repo::init_magic()?,
        })
    }

    async fn validate_repo<T>(repo_path: T) -> Result<Self>
    where
        T: AsRef<Path>,
    {
        let repo_path = repo_path.as_ref();

        // Create store
        let store_path = repo_path.join("store");
        if !store_path.is_dir() {
            return Err(Error {
                msg: format!(
                    "File store does not exist or is not a directory at {}.",
                    store_path.display()
                ),
                kind: ErrorKind::StoreFolder,
            });
        }

        // Create thumbnail store
        let thumbnail_path = repo_path.join("thumbnail");
        if !thumbnail_path.is_dir() {
            return Err(Error {
                msg: format!(
                    "Thumbnail store does not exist or is not a directory at {}.",
                    thumbnail_path.display()
                ),
                kind: ErrorKind::ThumbnailFolder,
            });
        }

        // Create DB
        Ok(Repo {
            path: repo_path.to_owned(),
            db: DB::new(repo_path.join("vorg.db")).await?,
            magic_cookie: Repo::init_magic()?,
        })
    }

    fn init_magic() -> Result<magic::Cookie> {
        let cookie =
            magic::Cookie::open(magic::CookieFlags::ERROR | magic::CookieFlags::MIME_TYPE)?;
        cookie.load::<&str>(&[])?;
        Ok(cookie)
    }

    /// Imports a file or folder into the vorg repo.
    ///
    /// This process MOVES the imported file into vorg store, generates thumbnails of it so that it
    /// can be previewed, and inserts metadata into the db.
    ///
    /// Initially, the imported file's title will be the file's filename, and will have a tag of
    /// "Incomplete" so that its metadata can be updated in future.
    ///
    /// If `path` points to a file, the file will be imported.
    /// If `path` points to a folder, all supported files within the folder will be recursively
    /// imported.
    ///
    /// # Errors
    ///
    /// If `file_path` points to a regular file,
    /// - `ErrorKind::FileNotFound` when the file to import cannot be found.
    /// - `ErrorKind::Unsupported` when the file to import has a currently unsupported type.
    /// - `ErrorKind::Duplicate` when the file to import already exists in repo.
    /// - `ErrorKind::IO` when import failed due to system IO error.
    ///
    /// If `file_path` points to a folder,
    /// Only `ErrorKind::FileNotFound` and `ErrorKind::IO` are returned. The other two types are
    /// suppressed. See stderr if those errors need to be known.
    pub async fn import<T>(&mut self, file_path: T) -> Result<()>
    where
        T: AsRef<Path>,
    {
        let file_path = file_path.as_ref();

        if !file_path.exists() {
            return Err(Error {
                msg: format!(
                    "The file to import cannot be found: {}.",
                    file_path.display()
                ),
                kind: ErrorKind::FileNotFound,
            });
        }

        if file_path.is_dir() {
            // Folder recursive import
            self.import_dir(file_path).await?;
        } else {
            // Single file
            self.import_file(file_path).await?;
        }

        Ok(())
    }

    async fn import_dir<T>(&mut self, dir: T) -> Result<()>
    where
        T: AsRef<Path>,
    {
        let dir = dir.as_ref().to_owned();
        let mut dir_stack = VecDeque::new();
        dir_stack.push_front(dir);
        while let Some(current_dir) = dir_stack.pop_front() {
            for entry in fs::read_dir(current_dir).expect("Error opening directory.") {
                let entry = entry.expect("Error getting entry in directory.");
                let path = entry.path();
                if path.is_dir() {
                    dir_stack.push_front(path);
                } else {
                    let Err(error) = self.import_file(&path).await else {
                        continue;
                    };
                    match error.kind {
                        ErrorKind::IO => {
                            // Do not suppress IO error, as those indicate import failure.
                            return Err(error);
                        }
                        _ => {
                            // Suppress all other errors, since those are either unsupported or
                            // duplicates.
                            eprintln!("Error encountered: {error}. Ignoring.");
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn import_file<T>(&mut self, file: T) -> Result<()>
    where
        T: AsRef<Path>,
    {
        let file = file.as_ref();

        // Check file type
        let mime_type = self
            .magic_cookie
            .file(file)
            .expect("Libmagic ffi should not fail.");
        let mime_result = SUPPORTED_MIMETYPES.get(mime_type.as_str());
        if mime_result.is_none() {
            return Err(Error {
                msg: format!(
                    "The file to import has an supported type: {}.",
                    file.display()
                ),
                kind: ErrorKind::Unsupported,
            });
        }
        let default_extension = *mime_result.unwrap();

        // Compute hash
        let hash = Repo::hash(file).unwrap();

        // Use the full file path as placeholder title.
        let title = file.to_string_lossy().into_owned();

        // Get extension
        let ext = file.extension().map_or_else(
            || String::from(default_extension),
            |filename| filename.to_string_lossy().into_owned(),
        );

        // Import into db
        // This will propagate `ErrorKind::Duplicate` if a duplicate is imported.
        self.db.import_file(&title, &ext, &hash).await?;

        // Prepare to move into store
        let store_subfolder = self.path.join("store").join(&hash[0..2]);
        let store_path = store_subfolder.join(format!("{}.{}", &hash[2..], ext));

        // Check/create store subfolder
        fs::create_dir(&store_subfolder)?;

        // Attempt rename first.
        // If source and destination are on different file systems, fallback to copy and remove.
        if let Err(error) = fs::rename(file, &store_path) {
            // TODO: when io_error_more is stablized, use ErrorKind::CrossesDevices instead.
            // This scenario cannot be easily tested. I just tried it and it seems to work.
            // Avoid importing files from across device boundries is the most prudent choice.
            if error.to_string().starts_with("Invalid cross-device link") {
                fs::copy(file, &store_path)?;
                fs::remove_file(file)?;
            } else {
                return Err(Error {
                    msg: error.to_string(),
                    kind: ErrorKind::IO,
                });
            }
        }

        // TODO: Generate thumbnail

        Ok(())
    }

    /// Get files that satisfy the given filter.
    ///
    /// TODO: Add filtering.
    ///
    pub async fn get_files(&mut self) -> Result<Vec<Item>> {
        self.db.get_items().await
    }

    /**
     * This function exhaustively checks the integrity of the repository.
     * Returns a textual description of the errors found, one error per line.
     * If the repo has no problems, returns an empty string.
     *
     * All errors are specified relative to the info found in db.
     * Three kinds of errors are possible:
     * store: having more or less files than in db.
     * hash: hash of the file found in store does not match what's stored in db.
     * ext: extension found in store is different in db
     * thumbnail: having thumbnails for more or less files than in db.
     *
     * This can be really slow on large repos.
     * Do not run regularly and do not run on UI thread.
     */
    pub async fn check_data_integrity(&mut self) -> Result<String> {
        let mut result = String::new();

        let db_files = self.db.get_items().await?;

        // Check store
        let mut store_files = Vec::new();
        let mut wrong_hash = Vec::new();
        Repo::check_store_folder(&self.path.join("store"), &mut store_files, &mut wrong_hash)?;

        // TODO: Check thumbnail

        // Process result
        store_files.sort();
        let mut i = 0;
        let mut j = 0;
        while i < db_files.len() && j < store_files.len() {
            let db_hash = &db_files[i].hash;
            let db_ext = &db_files[i].ext;
            let (store_hash, store_ext) = &store_files[j];
            if db_hash == store_hash {
                i += 1;
                j += 1;

                // Only check ext for full match
                if db_ext != store_ext {
                    result.push_str(
                        format!(
                            "ext: different extensions: {db_ext} in db but {store_ext} in store\n",
                        )
                        .as_str(),
                    );
                }

                continue;
            }
            if db_hash < store_hash {
                i += 1;
                result.push_str(format!("store: file not found in store: {db_hash}\n").as_str());
                continue;
            }
            // db_hash > store_hash
            j += 1;
            result.push_str(format!("store: redundant file in store: {store_hash}\n").as_str());
        }
        while i < db_files.len() {
            result.push_str(
                format!("store: file not found in store: {}\n", &db_files[i].hash).as_str(),
            );
            i += 1;
        }
        while j < store_files.len() {
            result.push_str(
                format!("store: redundant file in store: {}\n", &store_files[j].0).as_str(),
            );
            j += 1;
        }
        for error in wrong_hash {
            result.push_str(format!("hash: {error}\n").as_str());
        }
        // TODO: add thumbnail errors

        Ok(result)
    }

    fn check_store_folder<T>(
        dir_path: T,
        found_files: &mut Vec<(String, String)>,
        wrong_hash: &mut Vec<String>,
    ) -> Result<()>
    where
        T: AsRef<Path>,
    {
        for entry in fs::read_dir(dir_path).expect("Error opening directory.") {
            let entry = entry.expect("Error getting entry in directory.");
            let path = entry.path();
            if path.is_dir() {
                Repo::check_store_folder(&path, found_files, wrong_hash)?;
            } else {
                let expected_hash = path
                    .parent()
                    .expect("Store item must have a parent")
                    .file_name()
                    .expect("Store item parent must have a filename.")
                    .to_string_lossy()
                    + path
                        .file_stem()
                        .expect("Store item must have a filestem.")
                        .to_string_lossy();
                let expected_hash = expected_hash.to_string();

                // TODO: remove progress
                println!("Checking {expected_hash}");

                let real_hash = Repo::hash(&path)?;
                if expected_hash != real_hash {
                    wrong_hash.push(format!(
                        "Expected {expected_hash}, but real hash is {real_hash}"
                    ));
                }
                let ext = path
                    .extension()
                    .expect("Store item must have an extension.")
                    .to_string_lossy()
                    .to_string();
                found_files.push((expected_hash, ext));
            }
        }
        Ok(())
    }

    fn hash<T>(path: T) -> Result<String>
    where
        T: AsRef<Path>,
    {
        let mut file = fs::File::open(path)?;
        let mut hasher = Sha224::new();
        io::copy(&mut file, &mut hasher)?;
        let hash = hasher.finalize();
        Ok(hex::encode(hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestFixture<T>
    where
        T: AsRef<Path>,
    {
        path: T,
    }

    impl<T> TestFixture<T>
    where
        T: AsRef<Path>,
    {
        fn new(path: T) -> Self {
            TestFixture { path }
        }
    }

    impl<T> Drop for TestFixture<T>
    where
        T: AsRef<Path>,
    {
        fn drop(&mut self) {
            let path = self.path.as_ref();
            if path.is_dir() {
                fs::remove_dir_all(path).expect("Failed to teardown temp test directory.");
            } else {
                fs::remove_file(path).expect("Failed to teardown test file.");
            }
        }
    }

    //     #[test]
    //     async fn test_create_repo() {
    //         let repo_path = "temp/create_repo";
    //         let _f = TestFixture::new(repo_path);

    //         let result = Repo::new(repo_path).await;
    //         assert!(result.is_ok());

    //         // Make sure store exists
    //         let repo_path = Path::new(repo_path);
    //         let store_path = repo_path.join("store");
    //         assert!(store_path.is_dir());

    //         // Make sure thumbnail path exists
    //         let thumbnail_path = repo_path.join("thumbnail");
    //         assert!(thumbnail_path.is_dir());

    //         // Make sure database exists and passes validate db
    //         let db_path = repo_path.join("vorg.db");
    //         assert!(db_path.is_file());
    //         let test_db = DB::new(db_path).await;
    //         assert!(test_db.is_ok());
    //     }

    //     #[test]
    //     async fn test_create_repo_failed() {
    //         let repo_path = "temp/create_repo_failed";
    //         let _f = TestFixture::new(repo_path);

    //         fs::File::create(repo_path).unwrap();

    //         let result = Repo::new(repo_path).await;
    //         assert!(result.is_err());
    //         assert_eq!(
    //             result.unwrap_err().to_string(),
    //             "The selected path is not a folder."
    //         );
    //     }

    //     #[test]
    //     async fn test_validate_repo_valid() {
    //         let repo_path = "temp/create_validate_repo_valid";
    //         let _f = TestFixture::new(repo_path);

    //         // Create valid repo
    //         // TODO: do not depend on Repo::new
    //         {
    //             let result = Repo::new(repo_path).await;
    //             assert!(result.is_ok());
    //         }

    //         // Validate
    //         let result = Repo::new(repo_path).await;
    //         assert!(result.is_ok());
    //     }

    //     #[test]
    //     async fn test_validate_repo_invalid1() {
    //         let repo_path = "resources/repo/invalid-db";

    //         let result = Repo::new(repo_path).await;
    //         assert!(result.is_err());
    //         assert_eq!(result.unwrap_err().to_string(), "file is not a database");
    //     }

    //     #[test]
    //     async fn test_validate_repo_invalid2() {
    //         let repo_path = "resources/repo/invalid-store-not-dir";

    //         let result = Repo::new(repo_path).await;
    //         assert!(result.is_err());
    //         assert_eq!(
    //             result.unwrap_err().to_string(),
    //             "Store does not exist or is not a directory."
    //         );
    //     }

    //     #[test]
    //     async fn test_validate_repo_invalid3() {
    //         let repo_path = "resources/repo/invalid-thumbnail-not-dir";

    //         let result = Repo::new(repo_path).await;
    //         assert!(result.is_err());
    //         assert_eq!(
    //             result.unwrap_err().to_string(),
    //             "Thumbnail store does not exist or is not a directory."
    //         );
    //     }

    //     #[test]
    //     async fn test_import_file() {
    //         let repo_path = PathBuf::from("temp/repo_import_file");
    //         let _f = TestFixture::new(&repo_path);
    //         let video_path = PathBuf::from("temp/repo_import_file_videos");
    //         let _f2 = TestFixture::new(&video_path);
    //         fs::create_dir(&video_path).unwrap();

    //         // Make copy before importing
    //         let file_to_import = video_path.join("black.mp4");
    //         fs::copy("resources/video/black.mp4", &file_to_import).unwrap();

    //         // TODO: do not depend on Repo::new
    //         let original_file_size = file_to_import.metadata().unwrap().len();
    //         let mut repo = Repo::new(&repo_path).await.unwrap();
    //         let result = repo.import(&file_to_import).await;
    //         assert!(result.is_ok());

    //         // Verify store
    //         let expected_store_path = repo_path
    //             .join("store")
    //             .join("4e")
    //             .join("ffadeed3957d9dab1a645b9a7d01c18380d54e71d51148fdf84633.mp4");
    //         assert!(expected_store_path.exists());
    //         assert_eq!(
    //             original_file_size,
    //             expected_store_path.metadata().unwrap().len()
    //         );
    //         assert!(!file_to_import.exists());

    //         // Verify DB
    //         // let mut connection = SqliteConnection::connect(repo_path.()()("vorg.db"n.to_string().as_str()).await.unwrap();
    //         // let query = "
    //         // SELECT hash FROM items
    //         // ";
    //         // let results = sqlx::query(query).fetch_all(&mut connection).await.unwrap();
    //         // assert_eq!(results.len(), 1);
    //         // assert_eq!(
    //         //     statement.read::<String, _>(0).unwrap(),
    //         //     "4effadeed3957d9dab1a645b9a7d01c18380d54e71d51148fdf84633"
    //         // );
    //         // assert_eq!(statement.read::<String, _>(1).unwrap(), "black");
    //         // assert_eq!(statement.read::<String, _>(2).unwrap(), "mp4");
    //         // assert_eq!(statement.read::<i64, _>(3).unwrap(), 0);

    //         // let result = statement.next();
    //         // assert!(result.is_ok());
    //         // assert_eq!(result.unwrap(), sqlite::State::Done);

    //         // TODO: verify thumbnail

    //         // Test duplicate import
    //         fs::copy("resources/video/black.mp4", &file_to_import).unwrap();
    //         let result = repo.import(&file_to_import).await;
    //         assert!(result.is_err());
    //         assert_eq!(
    //             result.unwrap_err().to_string(),
    //             "The item to import already exists in the database."
    //         );

    //         // TODO: Give get_files an independent test
    //         let result = repo.get_files().await;
    //         assert!(result.is_ok());
    //         assert_eq!(result.unwrap().len(), 1);
    //     }

    //     #[test]
    //     async fn test_import_file_unsupported() {
    //         let repo_path = PathBuf::from("temp/repo_import_file_unsupported");
    //         let _f = TestFixture::new(&repo_path);

    //         let file_to_import = PathBuf::from("resources/video/fake-video.txt");
    //         let mut repo = Repo::new(&repo_path).await.unwrap();
    //         let result = repo.import(&file_to_import).await;
    //         assert!(result.is_err());
    //         assert_eq!(
    //             result.unwrap_err().to_string(),
    //             "File with type inode/x-empty is not supported."
    //         );
    //         assert!(file_to_import.exists());
    //     }

    //     #[test]
    //     async fn test_import_file_nonexistent() {
    //         let repo_path = PathBuf::from("temp/repo_import_file_nonexistent");
    //         let _f = TestFixture::new(&repo_path);

    //         let file_to_import = PathBuf::from("resources/video/no.mp4");
    //         let mut repo = Repo::new(&repo_path).await.unwrap();
    //         let result = repo.import(&file_to_import).await;
    //         assert!(result.is_err());
    //         assert_eq!(
    //             result.unwrap_err().to_string(),
    //             "The selected file does not exist."
    //         );
    //     }

    //     #[test]
    //     async fn test_import_file_subfolder_exists() {
    //         let repo_path = PathBuf::from("temp/repo_import_file_subfolder_exists");
    //         let _f = TestFixture::new(&repo_path);
    //         let video_path = PathBuf::from("temp/repo_import_file_subfolder_exists_video");
    //         let _f2 = TestFixture::new(&video_path);
    //         fs::create_dir(&video_path).unwrap();

    //         // Make copy before importing
    //         let file_to_import = video_path.join("black.mp4");
    //         fs::copy("resources/video/black.mp4", &file_to_import).unwrap();
    //         let original_file_size = file_to_import.metadata().unwrap().len();
    //         let mut repo = Repo::new(&repo_path).await.unwrap();

    //         // Create store subfolder
    //         fs::create_dir(repo_path.join("store").join("4e")).unwrap();
    //         fs::create_dir(repo_path.join("thumbnail").join("4e")).unwrap();

    //         // Import
    //         let result = repo.import(&file_to_import).await;
    //         assert!(result.is_ok());

    //         // Verify store
    //         let expected_store_path = repo_path
    //             .join("store")
    //             .join("4e")
    //             .join("ffadeed3957d9dab1a645b9a7d01c18380d54e71d51148fdf84633.mp4");
    //         assert!(expected_store_path.exists());
    //         assert_eq!(
    //             original_file_size,
    //             expected_store_path.metadata().unwrap().len()
    //         );
    //         assert!(!file_to_import.exists());

    //         // TODO: verify thumbnail
    //     }

    //     #[test]
    //     async fn test_import_file_store_corrupted() {
    //         let repo_path = PathBuf::from("temp/repo_import_file_corrupted");
    //         let _f = TestFixture::new(&repo_path);
    //         let video_path = PathBuf::from("temp/repo_import_file_corrupted_video");
    //         let _f2 = TestFixture::new(&video_path);
    //         fs::create_dir(&video_path).unwrap();

    //         // Make copy before importing
    //         let file_to_import = video_path.join("black.mp4");
    //         fs::copy("resources/video/black.mp4", &file_to_import).unwrap();
    //         let mut repo = Repo::new(&repo_path).await.unwrap();

    //         // Create store subfolder
    //         fs::File::create(repo_path.join("store").join("4e")).unwrap();

    //         // Import
    //         let result = repo.import(&file_to_import).await;
    //         assert!(result.is_err());
    //         assert_eq!(
    //             result.unwrap_err().to_string(),
    //             "Repo store is corrupted with regular files directly within."
    //         );
    //     }

    //     #[test]
    //     async fn test_import_folder() {
    //         let repo_path = PathBuf::from("temp/repo_import_dir");
    //         let _f = TestFixture::new(&repo_path);
    //         let video_path = PathBuf::from("temp/repo_import_dir_videos");
    //         let _f2 = TestFixture::new(&video_path);

    //         // Prepare video dir
    //         fs::create_dir_all(video_path.join("nested").join("another")).unwrap();
    //         fs::copy(
    //             "resources/video/black.mp4",
    //             "temp/repo_import_dir_videos/random title 1.mp4",
    //         )
    //         .unwrap();
    //         fs::copy(
    //             "resources/video/gray.mp4",
    //             "temp/repo_import_dir_videos/nested/random title 2.mp4",
    //         )
    //         .unwrap();
    //         fs::copy(
    //             "resources/video/large.mp4",
    //             "temp/repo_import_dir_videos/nested/another/random title 3.mp4",
    //         )
    //         .unwrap();
    //         fs::copy(
    //             "resources/video/white.mp4",
    //             "temp/repo_import_dir_videos/random title 4.mp4",
    //         )
    //         .unwrap();
    //         fs::copy(
    //             "resources/video/fake-video.txt",
    //             "temp/repo_import_dir_videos/fake video.txt",
    //         )
    //         .unwrap();

    //         // Prepare repo and import
    //         let mut repo = Repo::new(&repo_path).await.unwrap();
    //         let result = repo.import(&video_path).await;
    //         assert!(result.is_ok());

    //         // Verify non-video files are not touched
    //         assert!(PathBuf::from("temp/repo_import_dir_videos/fake video.txt").exists());

    //         // Verify
    //         let hashes = [
    //             "4effadeed3957d9dab1a645b9a7d01c18380d54e71d51148fdf84633",
    //             "50a04dc1cbd3d8edd5ad7acbcaad95362fe1c47c212f7b6b2b66d8bc",
    //             "effaa79355fe625a1df6e916b1c30a5f68ae76687dbd954d759353d6",
    //             "f9d939a70a8fbea1b6bde16c41fcbc1ce5ebe8002c7ccfaf791b891d",
    //         ];
    //         let mut titles = HashMap::new();
    //         titles.insert(
    //             "4effadeed3957d9dab1a645b9a7d01c18380d54e71d51148fdf84633",
    //             "random title 1",
    //         );
    //         titles.insert(
    //             "50a04dc1cbd3d8edd5ad7acbcaad95362fe1c47c212f7b6b2b66d8bc",
    //             "random title 2",
    //         );
    //         titles.insert(
    //             "effaa79355fe625a1df6e916b1c30a5f68ae76687dbd954d759353d6",
    //             "random title 3",
    //         );
    //         titles.insert(
    //             "f9d939a70a8fbea1b6bde16c41fcbc1ce5ebe8002c7ccfaf791b891d",
    //             "random title 4",
    //         );

    //         // Verify store
    //         for hash in hashes {
    //             let store_path = repo_path
    //                 .join("store")
    //                 .join(&hash[0..2])
    //                 .join(format!("{}.mp4", &hash[2..]));
    //             assert!(store_path.exists());
    //         }

    //         // Verify db
    //         // let connection = sqlite::open("temp/repo_import_dir/vorg.db").unwrap();
    //         // let query = "
    //         //     SELECT hash,title,ext,studio_id FROM items ORDER BY hash
    //         // ";
    //         // let mut statement = connection.prepare(query).unwrap();
    //         // let mut count = 0;
    //         // while let Ok(sqlite::State::Row) = statement.next() {
    //         //     assert_eq!(statement.read::<String, _>(0).unwrap(), hashes[count]);
    //         //     assert_eq!(
    //         //         statement.read::<String, _>(1).unwrap(),
    //         //         *titles.get(&hashes[count]).unwrap()
    //         //     );
    //         //     assert_eq!(statement.read::<String, _>(2).unwrap(), "mp4");
    //         //     assert_eq!(statement.read::<i64, _>(3).unwrap(), 0);
    //         //     count += 1;
    //         // }
    //         // assert_eq!(count, 4);

    //         // TODO: Verify thumbnail
    //     }

    //     #[test]
    //     async fn test_check_data_integrity() {
    //         let mut repo = Repo::new("resources/repo/db-not-store").await.unwrap();
    //         let result = repo.check_data_integrity().await.unwrap();
    //         assert_eq!(result, "store: file not found in store: 4effadeed3957d9dab1a645b9a7d01c18380d54e71d51148fdf84633
    // store: file not found in store: effaa79355fe625a1df6e916b1c30a5f68ae76687dbd954d759353d6
    // ");

    //         let mut repo = Repo::new("resources/repo/store-not-db").await.unwrap();
    //         let result = repo.check_data_integrity().await.unwrap();
    //         assert_eq!(result, "store: redundant file in store: 4effadeed3957d9dab1a645b9a7d01c18380d54e71d51148fdf84633
    // store: redundant file in store: effaa79355fe625a1df6e916b1c30a5f68ae76687dbd954d759353d6
    // ");

    //         let mut repo = Repo::new("resources/repo/wrong-hash-ext").await.unwrap();
    //         let result = repo.check_data_integrity().await.unwrap();
    //         assert_eq!(result, "ext: different extensions: avi in db but mp4 in store
    // hash: Expected 50a04dc1cbd3d8edd5ad7acbcaad95362fe1c47c212f7b6b2b66d8bd, but real hash is 50a04dc1cbd3d8edd5ad7acbcaad95362fe1c47c212f7b6b2b66d8bc
    // ");
    //     }

    //     #[test]
    //     async fn test_debug_fmt() {
    //         let repo_path = "temp/repo_debug_fmt";
    //         let _f = TestFixture::new(repo_path);

    //         let repo = Repo::new(repo_path).await.unwrap();
    //         let debug_fmt = format!("{repo:?}");
    //         assert!(
    //             debug_fmt.starts_with("Repo { db: Placeholder debug implementation for vorgrs::db::DB")
    //         );
    //     }
}
