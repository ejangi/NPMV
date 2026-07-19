#[cfg(test)]
mod tests {
    use crate::models::Release;
    use crate::rocket;
    use crate::routes::diff::{diff_files, strip_node_modules_regions, DiffQuery};
    use rocket::http::Status;

    #[test]
    fn test_diff_query_extract_versions() {
        let query = DiffQuery {
            version: Some(vec!["4.8.6".to_string(), "4.8.7".to_string()]),
            v: None,
            from: None,
            to: None,
            v1: None,
            v2: None,
            include_node_modules: Some(true),
            raw: Some(true),
        };
        assert_eq!(query.extract_versions(), vec!["4.8.6", "4.8.7"]);
        assert_eq!(query.include_node_modules, Some(true));
        assert_eq!(query.raw, Some(true));

        let query2 = DiffQuery {
            version: None,
            v: None,
            from: Some("1.0.0".to_string()),
            to: Some("1.0.1".to_string()),
            v1: None,
            v2: None,
            include_node_modules: None,
            raw: None,
        };
        assert_eq!(query2.extract_versions(), vec!["1.0.0", "1.0.1"]);
        assert_eq!(query2.include_node_modules, None);
        assert_eq!(query2.raw, None);
    }

    #[test]
    fn test_diff_files_text() {
        let old_content = b"hello\nworld\n";
        let new_content = b"hello\nrust\nworld\n";
        let diff = diff_files("test.txt", Some(old_content), Some(new_content), false).unwrap();
        assert!(diff.contains("--- a/test.txt"));
        assert!(diff.contains("+++ b/test.txt"));
        assert!(diff.contains("+rust"));
    }

    #[test]
    fn test_diff_files_binary() {
        let old_content = &[0u8, 1, 2, 3];
        let new_content = &[0u8, 1, 2, 4];
        let diff = diff_files("binary.bin", Some(old_content), Some(new_content), false).unwrap();
        assert!(diff.contains("Binary files a/binary.bin and b/binary.bin differ"));
    }

    #[test]
    fn test_strip_node_modules_regions() {
        let text = "//#region node_modules/@vue/shared/dist/shared.esm-bundler.js\nfunction foo() {}\n//#endregion\nlet x = 1;";
        let stripped = strip_node_modules_regions(text);
        assert!(!stripped.contains("function foo"));
        assert!(stripped.contains("let x = 1;"));
    }

    #[test]
    fn test_release_diff_links() {
        let mut releases = vec![
            Release {
                version: "1.0.0".to_string(),
                date_time: None,
                tarball: "".to_string(),
                shasum: "".to_string(),
                diff: None,
                diff_raw: None,
            },
            Release {
                version: "1.0.1".to_string(),
                date_time: None,
                tarball: "".to_string(),
                shasum: "".to_string(),
                diff: None,
                diff_raw: None,
            },
        ];

        for i in 1..releases.len() {
            let prev_version = releases[i - 1].version.clone();
            let curr_version = releases[i].version.clone();
            releases[i].diff = Some(format!(
                "/diff/react?version[]={}&version[]={}",
                prev_version, curr_version
            ));
            releases[i].diff_raw = Some(format!(
                "/diff/react?version[]={}&version[]={}&raw=true",
                prev_version, curr_version
            ));
        }

        assert_eq!(releases[0].diff, None);
        assert_eq!(
            releases[1].diff,
            Some("/diff/react?version[]=1.0.0&version[]=1.0.1".to_string())
        );
        assert_eq!(
            releases[1].diff_raw,
            Some("/diff/react?version[]=1.0.0&version[]=1.0.1&raw=true".to_string())
        );
    }

    #[tokio::test]
    async fn test_index_route() {
        use rocket::local::asynchronous::Client;
        let client = Client::untracked(rocket()).await.expect("valid rocket instance");
        let response = client.get("/").dispatch().await;
        assert_eq!(response.status(), Status::Ok);
    }

    #[tokio::test]
    async fn test_diff_missing_versions() {
        use rocket::local::asynchronous::Client;
        let client = Client::untracked(rocket()).await.expect("valid rocket instance");
        let response = client.get("/diff/react").dispatch().await;
        assert_eq!(response.status(), Status::BadRequest);
    }
}
