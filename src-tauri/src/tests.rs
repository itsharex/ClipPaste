#[cfg(test)]
mod tests {
    use crate::clipboard::{detect_subtype, truncate_utf8};

    // === detect_subtype tests ===

    #[test]
    fn detect_subtype_url_http() {
        assert_eq!(detect_subtype("http://example.com"), Some("url".to_string()));
    }

    #[test]
    fn detect_subtype_url_https() {
        assert_eq!(detect_subtype("https://example.com/path?q=1"), Some("url".to_string()));
    }

    #[test]
    fn detect_subtype_url_with_spaces_is_not_url() {
        assert_eq!(detect_subtype("https://example.com some text"), None);
    }

    #[test]
    fn detect_subtype_email() {
        assert_eq!(detect_subtype("user@example.com"), Some("email".to_string()));
    }

    #[test]
    fn detect_subtype_email_with_plus() {
        assert_eq!(detect_subtype("user+tag@example.com"), Some("email".to_string()));
    }

    #[test]
    fn detect_subtype_not_email_no_dot() {
        assert_eq!(detect_subtype("user@localhost"), None);
    }

    #[test]
    fn detect_subtype_not_email_spaces() {
        assert_eq!(detect_subtype("user @example.com"), None);
    }

    #[test]
    fn detect_subtype_color_hex_3() {
        assert_eq!(detect_subtype("#fff"), Some("color".to_string()));
    }

    #[test]
    fn detect_subtype_color_hex_6() {
        assert_eq!(detect_subtype("#ff00aa"), Some("color".to_string()));
    }

    #[test]
    fn detect_subtype_color_hex_8() {
        assert_eq!(detect_subtype("#ff00aaff"), Some("color".to_string()));
    }

    #[test]
    fn detect_subtype_color_rgb() {
        assert_eq!(detect_subtype("rgb(255, 0, 0)"), Some("color".to_string()));
    }

    #[test]
    fn detect_subtype_color_rgba() {
        assert_eq!(detect_subtype("rgba(255, 0, 0, 0.5)"), Some("color".to_string()));
    }

    #[test]
    fn detect_subtype_color_hsl() {
        assert_eq!(detect_subtype("hsl(120, 100%, 50%)"), Some("color".to_string()));
    }

    #[test]
    fn detect_subtype_path_windows() {
        assert_eq!(detect_subtype("C:\\Users\\test\\file.txt"), Some("path".to_string()));
    }

    #[test]
    fn detect_subtype_path_windows_forward_slash() {
        assert_eq!(detect_subtype("D:/Projects/code"), Some("path".to_string()));
    }

    #[test]
    fn detect_subtype_path_unix() {
        assert_eq!(detect_subtype("/usr/local/bin/app"), Some("path".to_string()));
    }

    #[test]
    fn detect_subtype_path_unix_with_spaces_is_none() {
        // Unix paths with spaces aren't detected (by design)
        assert_eq!(detect_subtype("/usr/local/my dir/app"), None);
    }

    #[test]
    fn detect_subtype_empty() {
        assert_eq!(detect_subtype(""), None);
    }

    #[test]
    fn detect_subtype_whitespace_only() {
        assert_eq!(detect_subtype("   "), None);
    }

    #[test]
    fn detect_subtype_plain_text() {
        assert_eq!(detect_subtype("Hello World"), None);
    }

    #[test]
    fn detect_subtype_number() {
        assert_eq!(detect_subtype("12345"), None);
    }

    #[test]
    fn detect_subtype_trimmed_url() {
        assert_eq!(detect_subtype("  https://example.com  "), Some("url".to_string()));
    }

    #[test]
    fn detect_subtype_invalid_hex_color() {
        // 4-char hex is not a valid color
        assert_eq!(detect_subtype("#abcd"), None);
    }

    #[test]
    fn detect_subtype_hex_with_non_hex_chars() {
        assert_eq!(detect_subtype("#gggggg"), None);
    }

    // === truncate_utf8 tests ===

    #[test]
    fn truncate_utf8_short_string() {
        assert_eq!(truncate_utf8("hello", 10), "hello");
    }

    #[test]
    fn truncate_utf8_exact_length() {
        assert_eq!(truncate_utf8("hello", 5), "hello");
    }

    #[test]
    fn truncate_utf8_long_string() {
        assert_eq!(truncate_utf8("hello world", 5), "hello");
    }

    #[test]
    fn truncate_utf8_empty() {
        assert_eq!(truncate_utf8("", 5), "");
    }

    #[test]
    fn truncate_utf8_multibyte() {
        // Vietnamese with combining characters
        let text = "Xin chào thế giới";
        let result = truncate_utf8(text, 8);
        assert_eq!(result, "Xin chào");
    }

    #[test]
    fn truncate_utf8_emoji() {
        let text = "Hello 🌍🌎🌏 World";
        let result = truncate_utf8(text, 8);
        assert_eq!(result, "Hello 🌍🌎");
    }

    #[test]
    fn truncate_utf8_zero_chars() {
        assert_eq!(truncate_utf8("hello", 0), "");
    }

    #[test]
    fn truncate_utf8_cjk() {
        let text = "日本語テスト";
        assert_eq!(truncate_utf8(text, 3), "日本語");
    }

    // === calculate_hash tests ===

    mod calculate_hash_tests {
        use crate::clipboard::calculate_hash;

        #[test]
        fn same_input_gives_same_hash() {
            let input = b"hello world";
            let hash1 = calculate_hash(input);
            let hash2 = calculate_hash(input);
            assert_eq!(hash1, hash2);
        }

        #[test]
        fn different_inputs_give_different_hashes() {
            let hash1 = calculate_hash(b"hello");
            let hash2 = calculate_hash(b"world");
            assert_ne!(hash1, hash2);
        }

        #[test]
        fn empty_input_gives_consistent_hash() {
            let hash1 = calculate_hash(b"");
            let hash2 = calculate_hash(b"");
            assert_eq!(hash1, hash2);
            // Empty input should still produce a valid hash
            assert!(!hash1.is_empty());
        }

        #[test]
        fn hash_is_hex_string_of_expected_length() {
            let hash = calculate_hash(b"test content");
            // SHA256 produces 32 bytes = 64 hex characters
            assert_eq!(hash.len(), 64, "SHA256 hex hash should be 64 characters, got {}", hash.len());
            // Verify all characters are valid hex
            assert!(hash.chars().all(|c| c.is_ascii_hexdigit()),
                "Hash should only contain hex characters, got: {}", hash);
        }
    }

    // === Search cache tests ===

    mod search_cache_tests {
        use crate::clipboard::{SEARCH_CACHE, add_to_search_cache, remove_from_search_cache};

        #[test]
        fn add_entry_verify_in_cache() {
            let uuid = "test-uuid-search-add";
            let preview = "some preview text";
            let folder_id = None;

            add_to_search_cache(uuid, preview, folder_id);

            let cache = SEARCH_CACHE.read();
            let found = cache.iter().any(|(u, p, f, _)| u == uuid && p == "some preview text" && *f == folder_id);
            assert!(found, "Entry should be found in search cache after add");

            // Cleanup
            drop(cache);
            remove_from_search_cache(uuid);
        }

        #[test]
        fn remove_entry_verify_gone() {
            let uuid = "test-uuid-search-remove";
            add_to_search_cache(uuid, "to be removed", None);

            // Verify it was added
            {
                let cache = SEARCH_CACHE.read();
                assert!(cache.iter().any(|(u, _, _, _)| u == uuid));
            }

            remove_from_search_cache(uuid);

            // Verify it was removed
            let cache = SEARCH_CACHE.read();
            assert!(!cache.iter().any(|(u, _, _, _)| u == uuid),
                "Entry should not be found in search cache after remove");
        }

        #[test]
        fn add_multiple_remove_one_others_present() {
            let uuid1 = "test-uuid-multi-1";
            let uuid2 = "test-uuid-multi-2";
            let uuid3 = "test-uuid-multi-3";

            add_to_search_cache(uuid1, "first item", Some(1));
            add_to_search_cache(uuid2, "second item", None);
            add_to_search_cache(uuid3, "third item", Some(2));

            // Remove only the second one
            remove_from_search_cache(uuid2);

            let cache = SEARCH_CACHE.read();
            assert!(cache.iter().any(|(u, _, _, _)| u == uuid1), "First entry should still be present");
            assert!(!cache.iter().any(|(u, _, _, _)| u == uuid2), "Second entry should be removed");
            assert!(cache.iter().any(|(u, _, _, _)| u == uuid3), "Third entry should still be present");

            // Cleanup
            drop(cache);
            remove_from_search_cache(uuid1);
            remove_from_search_cache(uuid3);
        }
    }

    // === Settings cache tests ===

    mod settings_cache_tests {
        use crate::clipboard::{SETTINGS_CACHE, get_cached_setting};

        #[test]
        fn get_cached_setting_returns_none_for_missing_key() {
            let result = get_cached_setting("nonexistent_key_for_test_12345");
            assert_eq!(result, None, "Should return None for a key that was never inserted");
        }

        #[test]
        fn get_cached_setting_returns_value_after_insert() {
            let key = "test_setting_key_abc";
            let value = "test_setting_value";

            // Insert directly into the cache
            {
                let mut cache = SETTINGS_CACHE.write();
                cache.insert(key.to_string(), value.to_string());
            }

            let result = get_cached_setting(key);
            assert_eq!(result, Some(value.to_string()),
                "Should return the inserted value");

            // Cleanup
            {
                let mut cache = SETTINGS_CACHE.write();
                cache.remove(key);
            }
        }
    }

    // === Icon cache tests (Windows only) ===

    #[cfg(target_os = "windows")]
    mod icon_cache_tests {
        use crate::clipboard::ICON_CACHE;

        #[test]
        fn icon_cache_write_and_read() {
            let app_name = "test_app_icon_cache.exe";
            let icon_data = Some("base64encodedicon".to_string());

            // Write to the cache
            {
                let mut cache = ICON_CACHE.lock();
                cache.insert(app_name.to_string(), icon_data.clone());
            }

            // Read from the cache
            {
                let cache = ICON_CACHE.lock();
                let result = cache.get(app_name);
                assert_eq!(result, Some(&icon_data),
                    "Should be able to read back the icon data that was written");
            }

            // Cleanup
            {
                let mut cache = ICON_CACHE.lock();
                cache.remove(app_name);
            }
        }
    }

    // === Integration tests ===

    mod integration_tests {
        use crate::clipboard::{
            SEARCH_CACHE, SETTINGS_CACHE,
            add_to_search_cache, remove_from_search_cache, get_cached_setting,
            detect_subtype, truncate_utf8, calculate_hash,
        };

        /// Test the search cache workflow: add entries, search with single/multi-word,
        /// remove entries, verify search results update correctly.
        #[test]
        fn search_cache_full_workflow() {
            // Use unique prefixes to avoid collision with other tests running in parallel
            let prefix = "integ_search_";
            let uuid1 = format!("{prefix}uuid-1");
            let uuid2 = format!("{prefix}uuid-2");
            let uuid3 = format!("{prefix}uuid-3");

            add_to_search_cache(&uuid1, "Hello World from Rust", Some(1));
            add_to_search_cache(&uuid2, "Goodbye World from TypeScript", None);
            add_to_search_cache(&uuid3, "Hello TypeScript project", Some(2));

            // Single word search: "hello" should match uuid1 and uuid3
            {
                let cache = SEARCH_CACHE.read();
                let results: Vec<&str> = cache.iter()
                    .filter(|(u, p, _, _)| u.starts_with(prefix) && p.contains("hello"))
                    .map(|(u, _, _, _)| u.as_str())
                    .collect();
                assert_eq!(results.len(), 2, "Single word 'hello' should match 2 entries");
                assert!(results.contains(&uuid1.as_str()));
                assert!(results.contains(&uuid3.as_str()));
            }

            // Multi-word AND search: "hello rust" should match only uuid1
            {
                let cache = SEARCH_CACHE.read();
                let words = vec!["hello", "rust"];
                let results: Vec<&str> = cache.iter()
                    .filter(|(u, p, _, _)| u.starts_with(prefix) && words.iter().all(|w| p.contains(w)))
                    .map(|(u, _, _, _)| u.as_str())
                    .collect();
                assert_eq!(results.len(), 1, "Multi-word 'hello rust' should match 1 entry");
                assert_eq!(results[0], uuid1.as_str());
            }

            // Multi-word AND search: "world from" should match uuid1 and uuid2
            {
                let cache = SEARCH_CACHE.read();
                let words = vec!["world", "from"];
                let results: Vec<&str> = cache.iter()
                    .filter(|(u, p, _, _)| u.starts_with(prefix) && words.iter().all(|w| p.contains(w)))
                    .map(|(u, _, _, _)| u.as_str())
                    .collect();
                assert_eq!(results.len(), 2, "Multi-word 'world from' should match 2 entries");
            }

            // Remove uuid2 and verify search updates
            remove_from_search_cache(&uuid2);
            {
                let cache = SEARCH_CACHE.read();
                let results: Vec<&str> = cache.iter()
                    .filter(|(u, p, _, _)| u.starts_with(prefix) && p.contains("world"))
                    .map(|(u, _, _, _)| u.as_str())
                    .collect();
                assert_eq!(results.len(), 1, "After removing uuid2, 'world' should match only uuid1");
                assert_eq!(results[0], uuid1.as_str());
            }

            // Search with no matches
            {
                let cache = SEARCH_CACHE.read();
                let results: Vec<&str> = cache.iter()
                    .filter(|(u, p, _, _)| u.starts_with(prefix) && p.contains("nonexistentxyz"))
                    .map(|(u, _, _, _)| u.as_str())
                    .collect();
                assert!(results.is_empty(), "Nonexistent query should return no results");
            }

            // Verify folder_id filtering works alongside text search
            {
                let cache = SEARCH_CACHE.read();
                let results: Vec<&str> = cache.iter()
                    .filter(|(u, p, fid, _)| u.starts_with(prefix) && p.contains("hello") && *fid == Some(1))
                    .map(|(u, _, _, _)| u.as_str())
                    .collect();
                assert_eq!(results.len(), 1, "Folder-filtered search should match 1 entry");
                assert_eq!(results[0], uuid1.as_str());
            }

            // Cleanup
            remove_from_search_cache(&uuid1);
            remove_from_search_cache(&uuid3);
        }

        /// Test detect_subtype covers edge cases and doesn't false-positive.
        #[test]
        fn subtype_detection_comprehensive() {
            // URL: text containing "http" but not a standalone URL should NOT match
            assert_eq!(detect_subtype("Check http://example.com for details"), None,
                "Text with embedded URL should not match (has spaces)");
            assert_eq!(detect_subtype("httpnotaurl"), None,
                "Text starting with 'http' but not a URL should not match");
            // Note: "https://" with no host still matches the URL pattern
            // (starts_with "https://" and no whitespace). This is by design — the
            // detector is intentionally simple and fast, not a full URL validator.
            assert_eq!(detect_subtype("https://"), Some("url".to_string()),
                "Bare scheme passes the simple starts_with + no-whitespace check");

            // URL: valid URLs
            assert_eq!(detect_subtype("https://example.com/path?q=1&b=2#frag"), Some("url".to_string()));
            assert_eq!(detect_subtype("http://localhost:3000"), Some("url".to_string()));

            // Email edge cases
            assert_eq!(detect_subtype("not@an@email.com"), None,
                "Multiple @ signs should not be detected as email");
            assert_eq!(detect_subtype("@example.com"), None,
                "Empty local part should not be detected as email");
            assert_eq!(detect_subtype("user@.com"), Some("email".to_string()),
                "Domain starting with dot still has a dot, so the simple check passes");

            // Path edge cases
            assert_eq!(detect_subtype("C:\\"), Some("path".to_string()),
                "Drive root should be detected as path");
            assert_eq!(detect_subtype("Z:/some/mixed/path"), Some("path".to_string()),
                "Mixed slashes should still detect as path");
            assert_eq!(detect_subtype("/"), None,
                "Single slash should not be detected as path (len <= 1)");
            assert_eq!(detect_subtype("/a"), Some("path".to_string()),
                "Minimal Unix path should be detected");

            // Color edge cases
            assert_eq!(detect_subtype("#000"), Some("color".to_string()));
            assert_eq!(detect_subtype("#12345"), None,
                "5-char hex is not a valid color length");
            assert_eq!(detect_subtype("hsla(120, 50%, 50%, 0.5)"), Some("color".to_string()));

            // Multiline text should not match any subtype
            assert_eq!(detect_subtype("line1\nline2"), None,
                "Multiline text should not match URL, email, or path");

            // Priority: URL wins over other patterns
            assert_eq!(detect_subtype("https://user@example.com"), Some("url".to_string()),
                "URL-shaped string with @ should be detected as URL, not email");
        }

        /// Test truncate_utf8 with mixed content always produces valid UTF-8.
        #[test]
        fn truncate_preserves_valid_utf8() {
            // Large string with mixed ASCII + multibyte (CJK, emoji, accented)
            let mixed = "Hello 世界! 🌍 café résumé 日本語テスト ñ";

            // Truncate at various points and verify result is valid UTF-8
            for max in 0..=mixed.chars().count() + 5 {
                let result = truncate_utf8(mixed, max);
                // result is &str, which is always valid UTF-8 in Rust, but verify length
                let char_count = result.chars().count();
                assert!(char_count <= max,
                    "Truncated to {} chars but got {} chars", max, char_count);
            }

            // Verify exact truncation points
            assert_eq!(truncate_utf8(mixed, 6), "Hello ");
            assert_eq!(truncate_utf8(mixed, 8), "Hello 世界");

            // Very large max should return entire string
            assert_eq!(truncate_utf8(mixed, 10000), mixed);

            // Zero should return empty
            assert_eq!(truncate_utf8(mixed, 0), "");

            // String with only multibyte chars
            let cjk = "日本語テスト";
            assert_eq!(truncate_utf8(cjk, 3), "日本語");
            assert_eq!(truncate_utf8(cjk, 6), cjk);
            assert_eq!(truncate_utf8(cjk, 100), cjk);

            // String with 4-byte emoji
            let emoji = "🎉🎊🎈🎁";
            assert_eq!(truncate_utf8(emoji, 2), "🎉🎊");
            assert_eq!(truncate_utf8(emoji, 0), "");
        }

        /// Test settings cache concurrent access from multiple threads.
        #[test]
        fn settings_cache_concurrent_access() {
            use std::thread;
            use std::sync::Arc;
            use std::sync::atomic::{AtomicBool, Ordering};

            let prefix = "integ_concurrent_";
            let error_found = Arc::new(AtomicBool::new(false));

            // Spawn writer threads
            let mut handles = vec![];
            for t in 0..4 {
                let key = format!("{prefix}key_{t}");
                let err = error_found.clone();
                handles.push(thread::spawn(move || {
                    for i in 0..50 {
                        let value = format!("value_{t}_{i}");
                        {
                            let mut cache = SETTINGS_CACHE.write();
                            cache.insert(key.clone(), value.clone());
                        }
                        // Immediately read back
                        let read = get_cached_setting(&key);
                        if read.is_none() {
                            err.store(true, Ordering::SeqCst);
                        }
                    }
                }));
            }

            // Spawn reader threads that read concurrently
            for _ in 0..4 {
                let err = error_found.clone();
                let prefix_owned = prefix.to_string();
                handles.push(thread::spawn(move || {
                    for _ in 0..100 {
                        let cache = SETTINGS_CACHE.read();
                        // Just verify the cache is readable and doesn't panic
                        let _count = cache.iter()
                            .filter(|(k, _)| k.starts_with(&prefix_owned))
                            .count();
                        drop(cache);
                        // Also test via get_cached_setting
                        let _ = get_cached_setting(&format!("{prefix_owned}key_0"));
                    }
                    // If we got here without panicking, concurrent reads succeeded
                    let _ = err; // silence unused warning
                }));
            }

            for h in handles {
                h.join().expect("Thread panicked during concurrent cache access");
            }

            assert!(!error_found.load(Ordering::SeqCst),
                "A write followed by read should always return Some (not None)");

            // Cleanup
            {
                let mut cache = SETTINGS_CACHE.write();
                let keys: Vec<String> = cache.keys()
                    .filter(|k| k.starts_with(prefix))
                    .cloned()
                    .collect();
                for k in keys {
                    cache.remove(&k);
                }
            }
        }

        /// Test calculate_hash determinism and collision resistance with similar inputs.
        #[test]
        fn hash_collision_resistance() {
            let mut hashes = std::collections::HashSet::new();

            // Generate hashes for many similar inputs
            for i in 0..500 {
                let input = format!("test input number {}", i);
                let hash = calculate_hash(input.as_bytes());
                assert_eq!(hash.len(), 64, "SHA256 should always produce 64 hex chars");
                assert!(hashes.insert(hash.clone()),
                    "Hash collision detected for input '{}': {}", input, hash);
            }

            // Single-byte differences should produce different hashes
            let hash_a = calculate_hash(b"a");
            let hash_b = calculate_hash(b"b");
            let hash_aa = calculate_hash(b"aa");
            assert_ne!(hash_a, hash_b, "Single char difference should produce different hashes");
            assert_ne!(hash_a, hash_aa, "Different lengths should produce different hashes");

            // Verify determinism: same input always gives same hash
            for _ in 0..10 {
                assert_eq!(calculate_hash(b"determinism test"), calculate_hash(b"determinism test"));
            }

            // Empty vs whitespace
            let hash_empty = calculate_hash(b"");
            let hash_space = calculate_hash(b" ");
            let hash_newline = calculate_hash(b"\n");
            assert_ne!(hash_empty, hash_space);
            assert_ne!(hash_space, hash_newline);

            // Verify total uniqueness of all 500 hashes
            assert_eq!(hashes.len(), 500, "All 500 inputs should produce unique hashes");
        }
    }

    // === Database integration tests ===

    mod database_tests {
        use crate::database::Database;

        /// Create a temporary in-memory database for testing
        async fn setup_test_db() -> Database {
            let temp_dir = std::env::temp_dir().join(format!("clippaste_test_{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&temp_dir).unwrap();
            let db_path = temp_dir.join("test.db");
            let db = Database::new(db_path.to_str().unwrap(), &temp_dir).await;
            db.migrate().await.expect("Migration should succeed");
            db
        }

        /// Helper: insert a text clip into the test database
        async fn insert_clip(db: &Database, uuid: &str, text: &str, folder_id: Option<i64>, is_pinned: bool) {
            let hash = crate::clipboard::calculate_hash(text.as_bytes());
            let preview = &text[..text.len().min(2000)];
            sqlx::query(
                "INSERT INTO clips (uuid, clip_type, content, text_preview, content_hash, folder_id, is_deleted, is_pinned, created_at, last_accessed)
                 VALUES (?, 'text', ?, ?, ?, ?, 0, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
            )
            .bind(uuid)
            .bind(text.as_bytes())
            .bind(preview)
            .bind(&hash)
            .bind(folder_id)
            .bind(is_pinned)
            .execute(&db.pool).await.unwrap();
        }

        /// Helper: count clips matching a WHERE clause
        async fn count_clips(db: &Database, where_clause: &str) -> i64 {
            let sql = format!("SELECT COUNT(*) FROM clips WHERE {}", where_clause);
            sqlx::query_scalar::<_, i64>(&sql)
                .fetch_one(&db.pool).await.unwrap()
        }

        // --- Migration tests ---

        #[tokio::test]
        async fn migrate_creates_all_tables() {
            let db = setup_test_db().await;

            // Verify all tables exist
            let tables: Vec<(String,)> = sqlx::query_as(
                "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
            ).fetch_all(&db.pool).await.unwrap();
            let table_names: Vec<&str> = tables.iter().map(|(n,)| n.as_str()).collect();

            assert!(table_names.contains(&"clips"), "clips table should exist");
            assert!(table_names.contains(&"folders"), "folders table should exist");
            assert!(table_names.contains(&"settings"), "settings table should exist");
            assert!(table_names.contains(&"ignored_apps"), "ignored_apps table should exist");
            assert!(table_names.contains(&"schema_version"), "schema_version table should exist");
        }

        #[tokio::test]
        async fn migrate_creates_indexes() {
            let db = setup_test_db().await;

            let indexes: Vec<(String,)> = sqlx::query_as(
                "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'"
            ).fetch_all(&db.pool).await.unwrap();
            let idx_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

            assert!(idx_names.contains(&"idx_clips_hash"), "content_hash index");
            assert!(idx_names.contains(&"idx_clips_folder"), "folder_id index");
            assert!(idx_names.contains(&"idx_clips_created"), "created_at index");
            assert!(idx_names.contains(&"idx_folders_name"), "unique folder name index");
            // is_deleted index should NOT exist (dropped in migration v4)
            assert!(!idx_names.contains(&"idx_clips_deleted_created"), "is_deleted index should be dropped");
        }

        #[tokio::test]
        async fn schema_version_is_latest() {
            let db = setup_test_db().await;
            let version: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM schema_version")
                .fetch_one(&db.pool).await.unwrap();
            assert_eq!(version, 4, "Schema version should be 4 after all migrations");
        }

        // --- CRUD tests ---

        #[tokio::test]
        async fn insert_and_query_clip() {
            let db = setup_test_db().await;
            insert_clip(&db, "test-uuid-1", "Hello World", None, false).await;

            let clip: (String, String) = sqlx::query_as(
                "SELECT uuid, text_preview FROM clips WHERE uuid = ?"
            ).bind("test-uuid-1").fetch_one(&db.pool).await.unwrap();

            assert_eq!(clip.0, "test-uuid-1");
            assert_eq!(clip.1, "Hello World");
        }

        #[tokio::test]
        async fn insert_and_query_folder() {
            let db = setup_test_db().await;

            sqlx::query("INSERT INTO folders (name, icon, color) VALUES (?, ?, ?)")
                .bind("Test Folder")
                .bind("📁")
                .bind("blue")
                .execute(&db.pool).await.unwrap();

            let folder: (String, Option<String>, Option<String>) = sqlx::query_as(
                "SELECT name, icon, color FROM folders WHERE name = ?"
            ).bind("Test Folder").fetch_one(&db.pool).await.unwrap();

            assert_eq!(folder.0, "Test Folder");
            assert_eq!(folder.1.as_deref(), Some("📁"));
            assert_eq!(folder.2.as_deref(), Some("blue"));
        }

        #[tokio::test]
        async fn folder_unique_name_constraint() {
            let db = setup_test_db().await;

            sqlx::query("INSERT INTO folders (name) VALUES (?)")
                .bind("Unique Name").execute(&db.pool).await.unwrap();

            let result = sqlx::query("INSERT INTO folders (name) VALUES (?)")
                .bind("Unique Name").execute(&db.pool).await;

            assert!(result.is_err(), "Duplicate folder name should fail");
            let err_msg = result.unwrap_err().to_string();
            assert!(err_msg.contains("UNIQUE"), "Error should mention UNIQUE constraint");
        }

        // --- Settings tests ---

        #[tokio::test]
        async fn settings_insert_and_read() {
            let db = setup_test_db().await;

            sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES ('theme', 'dark')")
                .execute(&db.pool).await.unwrap();

            let val = db.get_setting("theme").await.unwrap();
            assert_eq!(val, Some("dark".to_string()));
        }

        #[tokio::test]
        async fn settings_missing_key_returns_none() {
            let db = setup_test_db().await;
            let val = db.get_setting("nonexistent").await.unwrap();
            assert_eq!(val, None);
        }

        // --- Ignored apps tests ---

        #[tokio::test]
        async fn ignored_apps_add_remove() {
            let db = setup_test_db().await;

            db.add_ignored_app("notepad.exe").await.unwrap();
            db.add_ignored_app("calc.exe").await.unwrap();

            let apps = db.get_ignored_apps().await.unwrap();
            assert_eq!(apps.len(), 2);
            assert!(apps.contains(&"notepad.exe".to_string()));

            // Duplicate insert should not fail (INSERT OR IGNORE)
            db.add_ignored_app("notepad.exe").await.unwrap();
            let apps = db.get_ignored_apps().await.unwrap();
            assert_eq!(apps.len(), 2, "Duplicate should not create extra row");

            db.remove_ignored_app("notepad.exe").await.unwrap();
            let apps = db.get_ignored_apps().await.unwrap();
            assert_eq!(apps.len(), 1);
            assert!(!apps.contains(&"notepad.exe".to_string()));
        }

        #[tokio::test]
        async fn is_app_ignored_case_insensitive() {
            let db = setup_test_db().await;
            db.add_ignored_app("Notepad.EXE").await.unwrap();

            assert!(db.is_app_ignored("notepad.exe").await.unwrap(), "Case-insensitive match");
            assert!(db.is_app_ignored("NOTEPAD.EXE").await.unwrap(), "Case-insensitive match");
            assert!(!db.is_app_ignored("unknown.exe").await.unwrap(), "Non-ignored app");
        }

        // --- enforce_max_items tests ---

        #[tokio::test]
        async fn enforce_max_items_no_limit_does_nothing() {
            let db = setup_test_db().await;

            // Insert 5 clips, no max_items setting
            for i in 0..5 {
                insert_clip(&db, &format!("clip-{}", i), &format!("text {}", i), None, false).await;
            }

            db.enforce_max_items().await;
            let count = count_clips(&db, "1=1").await;
            assert_eq!(count, 5, "No max_items set → no clips deleted");
        }

        #[tokio::test]
        async fn enforce_max_items_trims_oldest() {
            let db = setup_test_db().await;

            // Set max_items = 3
            sqlx::query("INSERT INTO settings (key, value) VALUES ('max_items', '3')")
                .execute(&db.pool).await.unwrap();

            // Insert 5 clips with staggered timestamps
            for i in 0..5 {
                sqlx::query(
                    "INSERT INTO clips (uuid, clip_type, content, text_preview, content_hash, is_deleted, is_pinned, created_at, last_accessed)
                     VALUES (?, 'text', ?, ?, ?, 0, 0, datetime('now', ?), CURRENT_TIMESTAMP)"
                )
                .bind(format!("trim-{}", i))
                .bind(format!("text {}", i).as_bytes().to_vec())
                .bind(format!("text {}", i))
                .bind(format!("hash-{}", i))
                .bind(format!("-{} minutes", 5 - i)) // older clips first
                .execute(&db.pool).await.unwrap();
            }

            db.enforce_max_items().await;
            let count = count_clips(&db, "1=1").await;
            assert_eq!(count, 3, "Should trim to max_items=3");

            // Verify oldest were deleted (trim-0, trim-1 are oldest)
            let remaining: Vec<(String,)> = sqlx::query_as("SELECT uuid FROM clips ORDER BY created_at ASC")
                .fetch_all(&db.pool).await.unwrap();
            let uuids: Vec<&str> = remaining.iter().map(|(u,)| u.as_str()).collect();
            assert!(!uuids.contains(&"trim-0"), "Oldest clip should be deleted");
            assert!(!uuids.contains(&"trim-1"), "Second oldest should be deleted");
            assert!(uuids.contains(&"trim-4"), "Newest clip should remain");
        }

        #[tokio::test]
        async fn enforce_max_items_protects_pinned() {
            let db = setup_test_db().await;

            sqlx::query("INSERT INTO settings (key, value) VALUES ('max_items', '2')")
                .execute(&db.pool).await.unwrap();

            // Insert 3 unpinned + 2 pinned
            for i in 0..3 {
                insert_clip(&db, &format!("unpin-{}", i), &format!("unpinned {}", i), None, false).await;
            }
            for i in 0..2 {
                insert_clip(&db, &format!("pin-{}", i), &format!("pinned {}", i), None, true).await;
            }

            db.enforce_max_items().await;

            // Pinned clips should all survive
            let pinned_count = count_clips(&db, "is_pinned = 1").await;
            assert_eq!(pinned_count, 2, "All pinned clips must survive");

            // Unpinned should be trimmed to max_items=2
            let unpinned_count = count_clips(&db, "is_pinned = 0").await;
            assert_eq!(unpinned_count, 2, "Unpinned clips trimmed to max_items");
        }

        #[tokio::test]
        async fn enforce_max_items_protects_folder_clips() {
            let db = setup_test_db().await;

            sqlx::query("INSERT INTO settings (key, value) VALUES ('max_items', '1')")
                .execute(&db.pool).await.unwrap();

            // Create a folder
            sqlx::query("INSERT INTO folders (name) VALUES ('Test')")
                .execute(&db.pool).await.unwrap();
            let folder_id: i64 = sqlx::query_scalar("SELECT id FROM folders WHERE name = 'Test'")
                .fetch_one(&db.pool).await.unwrap();

            // Insert 3 unfiled + 2 in folder
            for i in 0..3 {
                insert_clip(&db, &format!("unfiled-{}", i), &format!("unfiled {}", i), None, false).await;
            }
            for i in 0..2 {
                insert_clip(&db, &format!("filed-{}", i), &format!("filed {}", i), Some(folder_id), false).await;
            }

            db.enforce_max_items().await;

            let folder_count = count_clips(&db, "folder_id IS NOT NULL").await;
            assert_eq!(folder_count, 2, "Folder clips must survive enforce_max_items");

            let unfiled_count = count_clips(&db, "folder_id IS NULL AND is_pinned = 0").await;
            assert_eq!(unfiled_count, 1, "Unfiled clips trimmed to max_items=1");
        }

        // --- WAL mode verification ---

        #[tokio::test]
        async fn wal_mode_enabled() {
            let db = setup_test_db().await;
            let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
                .fetch_one(&db.pool).await.unwrap();
            assert_eq!(mode.to_lowercase(), "wal", "Database should be in WAL mode");
        }

        #[tokio::test]
        async fn foreign_keys_enabled() {
            let db = setup_test_db().await;
            let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
                .fetch_one(&db.pool).await.unwrap();
            assert_eq!(fk, 1, "Foreign keys should be enabled");
        }

        // --- Cleanup orphan images test ---

        #[tokio::test]
        async fn cleanup_orphan_images_removes_untracked_files() {
            let db = setup_test_db().await;

            // Create orphan image file
            let orphan_path = db.images_dir.join("orphan_image.png");
            std::fs::write(&orphan_path, b"fake image data").unwrap();
            assert!(orphan_path.exists());

            db.cleanup_orphan_images().await;

            assert!(!orphan_path.exists(), "Orphan image should be deleted");
        }

        #[tokio::test]
        async fn cleanup_orphan_images_preserves_tracked_files() {
            let db = setup_test_db().await;

            // Insert image clip referencing a file
            let filename = "tracked_image.png";
            let file_path = db.images_dir.join(filename);
            std::fs::write(&file_path, b"real image data").unwrap();

            sqlx::query(
                "INSERT INTO clips (uuid, clip_type, content, text_preview, content_hash, is_deleted, created_at, last_accessed)
                 VALUES ('img-1', 'image', ?, '', 'hash123', 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
            ).bind(filename.as_bytes()).execute(&db.pool).await.unwrap();

            db.cleanup_orphan_images().await;

            assert!(file_path.exists(), "Tracked image should NOT be deleted");
        }
    }
}
