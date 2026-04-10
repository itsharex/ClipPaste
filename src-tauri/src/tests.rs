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
            let found = cache.get(uuid).map_or(false, |(p, f, _)| p == "some preview text" && *f == folder_id);
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
                assert!(cache.contains_key(uuid));
            }

            remove_from_search_cache(uuid);

            // Verify it was removed
            let cache = SEARCH_CACHE.read();
            assert!(!cache.contains_key(uuid),
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
            assert!(cache.contains_key(uuid1), "First entry should still be present");
            assert!(!cache.contains_key(uuid2), "Second entry should be removed");
            assert!(cache.contains_key(uuid3), "Third entry should still be present");

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

            // Write to the cache (LRU uses `put`)
            {
                let mut cache = ICON_CACHE.lock();
                cache.put(app_name.to_string(), icon_data.clone());
            }

            // Read from the cache
            {
                let mut cache = ICON_CACHE.lock();
                let result = cache.get(app_name);
                assert_eq!(result, Some(&icon_data),
                    "Should be able to read back the icon data that was written");
            }

            // Cleanup (LRU uses `pop`)
            {
                let mut cache = ICON_CACHE.lock();
                cache.pop(app_name);
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
                    .filter(|(u, (p, _, _))| u.starts_with(prefix) && p.contains("hello"))
                    .map(|(u, _)| u.as_str())
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
                    .filter(|(u, (p, _, _))| u.starts_with(prefix) && words.iter().all(|w| p.contains(w)))
                    .map(|(u, _)| u.as_str())
                    .collect();
                assert_eq!(results.len(), 1, "Multi-word 'hello rust' should match 1 entry");
                assert_eq!(results[0], uuid1.as_str());
            }

            // Multi-word AND search: "world from" should match uuid1 and uuid2
            {
                let cache = SEARCH_CACHE.read();
                let words = vec!["world", "from"];
                let results: Vec<&str> = cache.iter()
                    .filter(|(u, (p, _, _))| u.starts_with(prefix) && words.iter().all(|w| p.contains(w)))
                    .map(|(u, _)| u.as_str())
                    .collect();
                assert_eq!(results.len(), 2, "Multi-word 'world from' should match 2 entries");
            }

            // Remove uuid2 and verify search updates
            remove_from_search_cache(&uuid2);
            {
                let cache = SEARCH_CACHE.read();
                let results: Vec<&str> = cache.iter()
                    .filter(|(u, (p, _, _))| u.starts_with(prefix) && p.contains("world"))
                    .map(|(u, _)| u.as_str())
                    .collect();
                assert_eq!(results.len(), 1, "After removing uuid2, 'world' should match only uuid1");
                assert_eq!(results[0], uuid1.as_str());
            }

            // Search with no matches
            {
                let cache = SEARCH_CACHE.read();
                let results: Vec<&str> = cache.iter()
                    .filter(|(u, (p, _, _))| u.starts_with(prefix) && p.contains("nonexistentxyz"))
                    .map(|(u, _)| u.as_str())
                    .collect();
                assert!(results.is_empty(), "Nonexistent query should return no results");
            }

            // Verify folder_id filtering works alongside text search
            {
                let cache = SEARCH_CACHE.read();
                let results: Vec<&str> = cache.iter()
                    .filter(|(u, (p, fid, _))| u.starts_with(prefix) && p.contains("hello") && *fid == Some(1))
                    .map(|(u, _)| u.as_str())
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

    // === Sensitive content detection tests ===

    mod sensitive_detection_tests {
        use crate::clipboard::detect_sensitive;

        #[test]
        fn detect_aws_key() {
            assert_eq!(detect_sensitive("AKIAIOSFODNN7EXAMPLE"), Some("aws_key".to_string()));
        }

        #[test]
        fn detect_github_token() {
            assert_eq!(detect_sensitive("ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"), Some("github_token".to_string()));
            assert_eq!(detect_sensitive("gho_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"), Some("github_token".to_string()));
        }

        #[test]
        fn detect_stripe_key() {
            assert_eq!(detect_sensitive("sk_live_abcdef1234567890abcdef"), Some("stripe_key".to_string()));
            assert_eq!(detect_sensitive("sk_test_abcdef1234567890abcdef"), Some("stripe_key".to_string()));
        }

        #[test]
        fn detect_slack_token() {
            assert_eq!(detect_sensitive("xoxb-123456789-abcdef"), Some("slack_token".to_string()));
        }

        #[test]
        fn detect_private_key() {
            assert_eq!(detect_sensitive("-----BEGIN RSA PRIVATE KEY-----\nMIIE..."), Some("private_key".to_string()));
            assert_eq!(detect_sensitive("-----BEGIN PRIVATE KEY-----\nMIIE..."), Some("private_key".to_string()));
        }

        #[test]
        fn detect_jwt() {
            assert_eq!(detect_sensitive("eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U"),
                Some("jwt".to_string()));
        }

        #[test]
        fn detect_credit_card_visa() {
            assert_eq!(detect_sensitive("4111111111111111"), Some("credit_card".to_string()));
        }

        #[test]
        fn detect_credit_card_with_dashes() {
            assert_eq!(detect_sensitive("4111-1111-1111-1111"), Some("credit_card".to_string()));
        }

        #[test]
        fn detect_credit_card_with_spaces() {
            assert_eq!(detect_sensitive("4111 1111 1111 1111"), Some("credit_card".to_string()));
        }

        #[test]
        fn not_sensitive_plain_text() {
            assert_eq!(detect_sensitive("Hello, this is a normal message"), None);
        }

        #[test]
        fn not_sensitive_url() {
            assert_eq!(detect_sensitive("https://example.com/page"), None);
        }

        #[test]
        fn not_sensitive_code() {
            assert_eq!(detect_sensitive("const x = 42; function foo() { return x; }"), None);
        }

        #[test]
        fn not_sensitive_empty() {
            assert_eq!(detect_sensitive(""), None);
            assert_eq!(detect_sensitive("   "), None);
        }

        #[test]
        fn not_sensitive_short_number() {
            // 12 digits — too short for credit card
            assert_eq!(detect_sensitive("123456789012"), None);
        }

        #[test]
        fn not_sensitive_random_long_number() {
            // Fails Luhn check
            assert_eq!(detect_sensitive("1234567890123456"), None);
        }
    }

    // === Fuzzy search tests ===

    mod fuzzy_search_tests {
        use crate::commands::clips::fuzzy_contains;

        #[test]
        fn exact_match() {
            assert!(fuzzy_contains("hello world", "hello"));
        }

        #[test]
        fn subsequence_match() {
            assert!(fuzzy_contains("api_key_value", "apikey"));
        }

        #[test]
        fn camel_case_match() {
            assert!(fuzzy_contains("getapikey", "apikey"));
        }

        #[test]
        fn no_match() {
            assert!(!fuzzy_contains("hello", "xyz"));
        }

        #[test]
        fn empty_needle() {
            assert!(fuzzy_contains("anything", ""));
        }

        #[test]
        fn empty_haystack() {
            assert!(!fuzzy_contains("", "a"));
        }

        #[test]
        fn case_sensitive() {
            // fuzzy_contains is case-sensitive; search_clips lowercases both sides first
            assert!(fuzzy_contains("hello world", "hlo"));
            assert!(!fuzzy_contains("hello world", "HLO"));
        }

        #[test]
        fn single_char() {
            assert!(fuzzy_contains("abc", "a"));
            assert!(fuzzy_contains("abc", "c"));
            assert!(!fuzzy_contains("abc", "d"));
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
            assert!(table_names.contains(&"app_icons"), "app_icons table should exist");
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
            assert_eq!(version, 7, "Schema version should be 7 after all migrations");
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

        // --- v1.8.6 bug-fix tests: search cache invariants ---

        #[tokio::test]
        async fn refresh_search_cache_for_clip_self_heals_missing_entry() {
            use crate::clipboard::{SEARCH_CACHE, refresh_search_cache_for_clip};

            let db = setup_test_db().await;

            // Create a folder + clip filed inside it, with a note
            sqlx::query("INSERT INTO folders (uuid, name, position) VALUES ('selfheal-folder', 'SelfHeal', 0)")
                .execute(&db.pool).await.unwrap();
            let folder_id: i64 = sqlx::query_scalar("SELECT id FROM folders WHERE uuid='selfheal-folder'")
                .fetch_one(&db.pool).await.unwrap();
            sqlx::query(
                "INSERT INTO clips (uuid, clip_type, content, text_preview, content_hash, folder_id, note, is_deleted, is_pinned, created_at, last_accessed)
                 VALUES ('selfheal-clip', 'text', 'Docker Compose', 'Docker Compose', 'hash-selfheal', ?, 'My Note', 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
            ).bind(folder_id).execute(&db.pool).await.unwrap();

            // Simulate the buggy state: cache is missing the entry
            SEARCH_CACHE.write().remove("selfheal-clip");
            assert!(SEARCH_CACHE.read().get("selfheal-clip").is_none(), "Precondition: cache empty");

            // Trigger self-heal (what the re-copy dedup branch now calls)
            refresh_search_cache_for_clip(&db.pool, "selfheal-clip", "Docker Compose").await;

            // Verify entry is back, with correct folder_id, lowercased preview, and lowercased note
            let cache = SEARCH_CACHE.read();
            let entry = cache.get("selfheal-clip").expect("Entry must be re-inserted after self-heal");
            assert_eq!(entry.0, "docker compose", "preview should be lowercased");
            assert_eq!(entry.1, Some(folder_id), "folder_id must be loaded from DB");
            assert_eq!(entry.2, "my note", "note should be lowercased");
        }

        #[tokio::test]
        async fn enforce_auto_delete_clears_search_cache_entries() {
            use crate::clipboard::{SEARCH_CACHE, add_to_search_cache};

            let db = setup_test_db().await;

            // Configure auto_delete_days = 7
            sqlx::query("INSERT INTO settings (key, value) VALUES ('auto_delete_days', '7')")
                .execute(&db.pool).await.unwrap();

            // Insert one old clip (>7 days) and one fresh clip
            sqlx::query(
                "INSERT INTO clips (uuid, clip_type, content, text_preview, content_hash, is_deleted, is_pinned, created_at, last_accessed)
                 VALUES ('autodel-old', 'text', 'old', 'old', 'hash-old', 0, 0, datetime('now', '-30 days'), CURRENT_TIMESTAMP)"
            ).execute(&db.pool).await.unwrap();
            sqlx::query(
                "INSERT INTO clips (uuid, clip_type, content, text_preview, content_hash, is_deleted, is_pinned, created_at, last_accessed)
                 VALUES ('autodel-fresh', 'text', 'fresh', 'fresh', 'hash-fresh', 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
            ).execute(&db.pool).await.unwrap();

            // Seed cache with both
            add_to_search_cache("autodel-old", "old", None);
            add_to_search_cache("autodel-fresh", "fresh", None);
            assert!(SEARCH_CACHE.read().contains_key("autodel-old"));
            assert!(SEARCH_CACHE.read().contains_key("autodel-fresh"));

            db.enforce_auto_delete().await;

            // DB row is gone
            let old_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM clips WHERE uuid='autodel-old'")
                .fetch_one(&db.pool).await.unwrap();
            assert_eq!(old_count, 0, "Old clip should be deleted from DB");

            // Cache should NOT contain the deleted clip (rebuilt by load_search_cache)
            let cache = SEARCH_CACHE.read();
            assert!(!cache.contains_key("autodel-old"), "Stale UUID must be evicted from cache");
            assert!(cache.contains_key("autodel-fresh"), "Fresh clip should remain in cache");
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

    // === Sync protocol tests ===

    mod sync_protocol_tests {
        use crate::database::Database;
        use crate::sync::drive::DriveClient;
        use crate::sync::models::*;
        use crate::sync::protocol::{SyncState, SyncDelta, SyncReport, apply_delta, build_full_state};

        /// Create a temporary test database with all migrations applied
        async fn setup_test_db() -> Database {
            let temp_dir = std::env::temp_dir().join(format!("clippaste_sync_test_{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&temp_dir).unwrap();
            let db_path = temp_dir.join("test.db");
            let db = Database::new(db_path.to_str().unwrap(), &temp_dir).await;
            db.migrate().await.expect("Migration should succeed");
            db
        }

        /// Create a DriveClient with a fake token (no HTTP calls for text-only tests)
        fn fake_drive() -> DriveClient {
            DriveClient::new("fake-token-for-testing")
        }

        /// Helper: insert a text clip with explicit uuid, content, hash, and timestamps
        async fn insert_clip_full(
            db: &Database,
            uuid: &str,
            text: &str,
            hash: &str,
            folder_id: Option<i64>,
            created_at: &str,
            updated_at: &str,
        ) {
            sqlx::query(
                "INSERT INTO clips (uuid, clip_type, content, text_preview, content_hash, folder_id,
                        is_deleted, is_pinned, is_sensitive, paste_count, created_at, last_accessed, updated_at)
                 VALUES (?, 'text', ?, ?, ?, ?, 0, 0, 0, 0, ?, ?, ?)"
            )
            .bind(uuid)
            .bind(text.as_bytes())
            .bind(&text[..text.len().min(2000)])
            .bind(hash)
            .bind(folder_id)
            .bind(created_at)
            .bind(updated_at)
            .bind(updated_at)
            .execute(&db.pool).await.unwrap();
        }

        /// Helper: insert a folder with explicit uuid, name, and timestamps
        async fn insert_folder_full(
            db: &Database,
            uuid: &str,
            name: &str,
            position: i64,
            created_at: &str,
            updated_at: &str,
        ) -> i64 {
            sqlx::query(
                "INSERT INTO folders (uuid, name, icon, color, position, created_at, updated_at)
                 VALUES (?, ?, NULL, NULL, ?, ?, ?)"
            )
            .bind(uuid)
            .bind(name)
            .bind(position)
            .bind(created_at)
            .bind(updated_at)
            .execute(&db.pool).await.unwrap();

            sqlx::query_scalar::<_, i64>("SELECT id FROM folders WHERE uuid = ?")
                .bind(uuid)
                .fetch_one(&db.pool).await.unwrap()
        }

        /// Helper: create a SyncClip for testing (text type)
        fn make_sync_clip(uuid: &str, text: &str, hash: &str, folder_uuid: Option<&str>, updated_at: &str) -> SyncClip {
            SyncClip {
                uuid: uuid.to_string(),
                clip_type: "text".to_string(),
                text_preview: text.to_string(),
                content_hash: hash.to_string(),
                folder_uuid: folder_uuid.map(|s| s.to_string()),
                source_app: None,
                metadata: None,
                subtype: None,
                note: None,
                paste_count: 0,
                is_pinned: false,
                is_sensitive: false,
                created_at: updated_at.to_string(),
                updated_at: updated_at.to_string(),
                text_content: Some(text.to_string()),
            }
        }

        /// Helper: create a SyncFolder for testing
        fn make_sync_folder(uuid: &str, name: &str, position: i64, updated_at: &str) -> SyncFolder {
            SyncFolder {
                uuid: uuid.to_string(),
                name: name.to_string(),
                icon: None,
                color: None,
                position,
                created_at: updated_at.to_string(),
                updated_at: updated_at.to_string(),
            }
        }

        /// Helper: create a SyncDelta for testing
        fn make_delta(
            clips: Vec<SyncClip>,
            folders: Vec<SyncFolder>,
            tombstones: Vec<Tombstone>,
        ) -> SyncDelta {
            SyncDelta {
                clips,
                folders,
                tombstones,
                device_id: "test-device-remote".to_string(),
                created_at: "2024-06-01T00:00:00Z".to_string(),
            }
        }

        // ──────────────────────────────────────────────
        // A. Serialization tests
        // ──────────────────────────────────────────────

        #[test]
        fn sync_state_round_trip_serialize() {
            let state = SyncState {
                clips: vec![make_sync_clip("c1", "hello", "hash1", None, "2024-01-01T00:00:00Z")],
                folders: vec![make_sync_folder("f1", "Work", 0, "2024-01-01T00:00:00Z")],
                tombstones: vec![Tombstone {
                    uuid: "t1".to_string(),
                    entity_type: "clip".to_string(),
                    deleted_at: "2024-01-01T00:00:00Z".to_string(),
                }],
                device_id: "device-a".to_string(),
                updated_at: "2024-01-01T12:00:00Z".to_string(),
            };

            let json = serde_json::to_string(&state).expect("serialize SyncState");
            let deserialized: SyncState = serde_json::from_str(&json).expect("deserialize SyncState");

            assert_eq!(deserialized.clips.len(), 1);
            assert_eq!(deserialized.clips[0].uuid, "c1");
            assert_eq!(deserialized.clips[0].text_preview, "hello");
            assert_eq!(deserialized.folders.len(), 1);
            assert_eq!(deserialized.folders[0].name, "Work");
            assert_eq!(deserialized.tombstones.len(), 1);
            assert_eq!(deserialized.tombstones[0].uuid, "t1");
            assert_eq!(deserialized.device_id, "device-a");
            assert_eq!(deserialized.updated_at, "2024-01-01T12:00:00Z");
        }

        #[test]
        fn sync_delta_round_trip_serialize() {
            let delta = SyncDelta {
                clips: vec![make_sync_clip("c2", "world", "hash2", Some("f1"), "2024-02-01T00:00:00Z")],
                folders: vec![],
                tombstones: vec![],
                device_id: "device-b".to_string(),
                created_at: "2024-02-01T00:00:00Z".to_string(),
            };

            let json = serde_json::to_string(&delta).expect("serialize SyncDelta");
            let deserialized: SyncDelta = serde_json::from_str(&json).expect("deserialize SyncDelta");

            assert_eq!(deserialized.clips.len(), 1);
            assert_eq!(deserialized.clips[0].folder_uuid, Some("f1".to_string()));
            assert_eq!(deserialized.device_id, "device-b");
        }

        #[test]
        fn sync_state_empty_collections() {
            let state = SyncState::default();

            let json = serde_json::to_string(&state).expect("serialize empty SyncState");
            let deserialized: SyncState = serde_json::from_str(&json).expect("deserialize empty SyncState");

            assert!(deserialized.clips.is_empty());
            assert!(deserialized.folders.is_empty());
            assert!(deserialized.tombstones.is_empty());
            assert!(deserialized.device_id.is_empty());
        }

        #[test]
        fn sync_delta_with_tombstones() {
            let delta = SyncDelta {
                clips: vec![],
                folders: vec![],
                tombstones: vec![
                    Tombstone { uuid: "del-1".to_string(), entity_type: "clip".to_string(), deleted_at: "2024-03-01T00:00:00Z".to_string() },
                    Tombstone { uuid: "del-2".to_string(), entity_type: "folder".to_string(), deleted_at: "2024-03-02T00:00:00Z".to_string() },
                ],
                device_id: "device-c".to_string(),
                created_at: "2024-03-01T00:00:00Z".to_string(),
            };

            let json = serde_json::to_string(&delta).expect("serialize delta with tombstones");
            let deserialized: SyncDelta = serde_json::from_str(&json).expect("deserialize");

            assert_eq!(deserialized.tombstones.len(), 2);
            assert_eq!(deserialized.tombstones[0].entity_type, "clip");
            assert_eq!(deserialized.tombstones[1].entity_type, "folder");
        }

        // ──────────────────────────────────────────────
        // B. apply_delta — clip merge tests
        // ──────────────────────────────────────────────

        #[tokio::test]
        async fn apply_delta_inserts_new_clip() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            let delta = make_delta(
                vec![make_sync_clip("new-clip-1", "Hello from remote", "hash-new-1", None, "2024-01-15T00:00:00Z")],
                vec![],
                vec![],
            );

            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            // Verify clip was inserted
            let row: Option<(String, String)> = sqlx::query_as(
                "SELECT uuid, text_preview FROM clips WHERE uuid = 'new-clip-1'"
            ).fetch_optional(&db.pool).await.unwrap();

            assert!(row.is_some(), "New clip should be inserted");
            let (uuid, preview) = row.unwrap();
            assert_eq!(uuid, "new-clip-1");
            assert_eq!(preview, "Hello from remote");
            assert_eq!(report.pulled_clips, 1);
        }

        #[tokio::test]
        async fn apply_delta_updates_clip_when_remote_is_newer() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            // Insert local clip with older timestamp
            insert_clip_full(&db, "clip-update-1", "old text", "hash-u1", None, "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;

            // Remote delta with newer timestamp
            let delta = make_delta(
                vec![make_sync_clip("clip-update-1", "updated text", "hash-u1-new", None, "2024-01-02T00:00:00Z")],
                vec![],
                vec![],
            );

            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            let preview: String = sqlx::query_scalar(
                "SELECT text_preview FROM clips WHERE uuid = 'clip-update-1'"
            ).fetch_one(&db.pool).await.unwrap();

            assert_eq!(preview, "updated text", "Clip should be updated to remote version");
            assert_eq!(report.pulled_clips, 1);
        }

        #[tokio::test]
        async fn apply_delta_skips_clip_when_local_is_newer() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            // Insert local clip with newer timestamp
            insert_clip_full(&db, "clip-skip-1", "local newer text", "hash-s1", None, "2024-01-02T00:00:00Z", "2024-01-02T00:00:00Z").await;

            // Remote delta with older timestamp
            let delta = make_delta(
                vec![make_sync_clip("clip-skip-1", "remote older text", "hash-s1-old", None, "2024-01-01T00:00:00Z")],
                vec![],
                vec![],
            );

            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            let preview: String = sqlx::query_scalar(
                "SELECT text_preview FROM clips WHERE uuid = 'clip-skip-1'"
            ).fetch_one(&db.pool).await.unwrap();

            assert_eq!(preview, "local newer text", "Local newer clip should NOT be overwritten");
            assert_eq!(report.pulled_clips, 0);
        }

        #[tokio::test]
        async fn apply_delta_content_hash_dedup_adopts_remote_uuid() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            // Insert local clip with UUID "local-1" and hash "shared-hash"
            insert_clip_full(&db, "local-1", "same content", "shared-hash", None, "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;

            // Remote delta with different UUID but same content hash
            let delta = make_delta(
                vec![make_sync_clip("remote-1", "same content", "shared-hash", None, "2024-01-02T00:00:00Z")],
                vec![],
                vec![],
            );

            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            // Local clip should now have the remote UUID
            let old_exists: Option<String> = sqlx::query_scalar(
                "SELECT uuid FROM clips WHERE uuid = 'local-1'"
            ).fetch_optional(&db.pool).await.unwrap();
            assert!(old_exists.is_none(), "Old local UUID should no longer exist");

            let new_exists: Option<String> = sqlx::query_scalar(
                "SELECT uuid FROM clips WHERE uuid = 'remote-1'"
            ).fetch_optional(&db.pool).await.unwrap();
            assert!(new_exists.is_some(), "Remote UUID should now exist in DB");

            assert_eq!(report.pulled_clips, 1);
        }

        #[tokio::test]
        async fn apply_delta_inserts_new_folder() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            let delta = make_delta(
                vec![],
                vec![make_sync_folder("folder-new-1", "Remote Folder", 0, "2024-01-15T00:00:00Z")],
                vec![],
            );

            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            let name: Option<String> = sqlx::query_scalar(
                "SELECT name FROM folders WHERE uuid = 'folder-new-1'"
            ).fetch_optional(&db.pool).await.unwrap();

            assert_eq!(name, Some("Remote Folder".to_string()), "New folder should be inserted");
            assert_eq!(report.pulled_folders, 1);
        }

        #[tokio::test]
        async fn apply_delta_updates_folder_when_remote_is_newer() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            // Insert local folder with older timestamp
            insert_folder_full(&db, "folder-upd-1", "Work", 0, "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;

            // Remote delta with newer timestamp and updated properties
            let mut remote_folder = make_sync_folder("folder-upd-1", "Work", 5, "2024-01-02T00:00:00Z");
            remote_folder.color = Some("blue".to_string());

            let delta = make_delta(vec![], vec![remote_folder], vec![]);

            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            let (position, color): (i64, Option<String>) = sqlx::query_as(
                "SELECT position, color FROM folders WHERE uuid = 'folder-upd-1'"
            ).fetch_one(&db.pool).await.unwrap();

            assert_eq!(position, 5, "Folder position should be updated");
            assert_eq!(color, Some("blue".to_string()), "Folder color should be updated");
            assert_eq!(report.pulled_folders, 1);
        }

        #[tokio::test]
        async fn apply_delta_skips_folder_when_local_is_newer() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            insert_folder_full(&db, "folder-skip-1", "Projects", 0, "2024-01-02T00:00:00Z", "2024-01-02T00:00:00Z").await;

            let delta = make_delta(
                vec![],
                vec![make_sync_folder("folder-skip-1", "Projects", 10, "2024-01-01T00:00:00Z")],
                vec![],
            );

            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            let position: i64 = sqlx::query_scalar(
                "SELECT position FROM folders WHERE uuid = 'folder-skip-1'"
            ).fetch_one(&db.pool).await.unwrap();

            assert_eq!(position, 0, "Folder should NOT be updated when local is newer");
            assert_eq!(report.pulled_folders, 0);
        }

        #[tokio::test]
        async fn apply_delta_same_name_folder_reconciliation() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            // Insert local folder with different UUID but same name
            insert_folder_full(&db, "local-folder-uuid", "Work", 0, "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;

            // Remote delta has a folder with different UUID but same name "Work"
            let delta = make_delta(
                vec![],
                vec![make_sync_folder("remote-folder-uuid", "Work", 3, "2024-01-02T00:00:00Z")],
                vec![],
            );

            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            // The local folder should now have the remote UUID
            let old_exists: Option<String> = sqlx::query_scalar(
                "SELECT uuid FROM folders WHERE uuid = 'local-folder-uuid'"
            ).fetch_optional(&db.pool).await.unwrap();
            assert!(old_exists.is_none(), "Old local folder UUID should no longer exist");

            let new_uuid: Option<String> = sqlx::query_scalar(
                "SELECT uuid FROM folders WHERE uuid = 'remote-folder-uuid'"
            ).fetch_optional(&db.pool).await.unwrap();
            assert!(new_uuid.is_some(), "Remote folder UUID should now exist");

            assert_eq!(report.pulled_folders, 1);
        }

        #[tokio::test]
        async fn apply_delta_tombstone_deletes_clip() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            // Insert a clip that will be tombstoned
            insert_clip_full(&db, "clip-to-delete", "doomed text", "hash-doom", None, "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;

            let delta = make_delta(
                vec![],
                vec![],
                vec![Tombstone {
                    uuid: "clip-to-delete".to_string(),
                    entity_type: "clip".to_string(),
                    deleted_at: "2024-01-15T00:00:00Z".to_string(),
                }],
            );

            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            let exists: Option<String> = sqlx::query_scalar(
                "SELECT uuid FROM clips WHERE uuid = 'clip-to-delete'"
            ).fetch_optional(&db.pool).await.unwrap();

            assert!(exists.is_none(), "Tombstoned clip should be deleted");
            assert_eq!(report.deleted, 1);
        }

        #[tokio::test]
        async fn apply_delta_tombstone_deletes_folder_and_unfiles_clips() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            // Insert a folder and a clip inside it
            let folder_id = insert_folder_full(&db, "folder-to-delete", "Doomed Folder", 0, "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;
            insert_clip_full(&db, "clip-in-folder", "inside doomed folder", "hash-inf", Some(folder_id), "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;

            let delta = make_delta(
                vec![],
                vec![],
                vec![Tombstone {
                    uuid: "folder-to-delete".to_string(),
                    entity_type: "folder".to_string(),
                    deleted_at: "2024-01-15T00:00:00Z".to_string(),
                }],
            );

            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            // Folder should be deleted
            let folder_exists: Option<String> = sqlx::query_scalar(
                "SELECT uuid FROM folders WHERE uuid = 'folder-to-delete'"
            ).fetch_optional(&db.pool).await.unwrap();
            assert!(folder_exists.is_none(), "Tombstoned folder should be deleted");

            // Clip should still exist but with folder_id = NULL
            let clip_folder: Option<Option<i64>> = sqlx::query_scalar(
                "SELECT folder_id FROM clips WHERE uuid = 'clip-in-folder'"
            ).fetch_optional(&db.pool).await.unwrap();
            assert!(clip_folder.is_some(), "Clip should still exist after folder deletion");
            assert_eq!(clip_folder.unwrap(), None, "Clip should be unfiled (folder_id = NULL)");

            assert_eq!(report.deleted, 1);
        }

        #[tokio::test]
        async fn apply_delta_folder_tombstone_clears_search_cache_folder_id() {
            use crate::clipboard::{SEARCH_CACHE, add_to_search_cache};

            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            // Insert folder + 2 clips inside it
            let folder_id = insert_folder_full(&db, "tomb-cache-folder", "Docker", 0, "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;
            insert_clip_full(&db, "tomb-clip-1", "docker compose up", "hash-tc1", Some(folder_id), "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;
            insert_clip_full(&db, "tomb-clip-2", "docker ps", "hash-tc2", Some(folder_id), "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;

            // Seed search cache with the folder_id (mirrors what load_search_cache would do)
            add_to_search_cache("tomb-clip-1", "docker compose up", Some(folder_id));
            add_to_search_cache("tomb-clip-2", "docker ps", Some(folder_id));
            assert_eq!(SEARCH_CACHE.read().get("tomb-clip-1").unwrap().1, Some(folder_id));

            // Apply folder tombstone
            let delta = make_delta(
                vec![],
                vec![],
                vec![Tombstone {
                    uuid: "tomb-cache-folder".to_string(),
                    entity_type: "folder".to_string(),
                    deleted_at: "2024-02-01T00:00:00Z".to_string(),
                }],
            );
            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            // Cache entries should now have folder_id = None (matching DB state)
            let cache = SEARCH_CACHE.read();
            assert_eq!(cache.get("tomb-clip-1").unwrap().1, None, "Cache folder_id must be cleared after folder tombstone");
            assert_eq!(cache.get("tomb-clip-2").unwrap().1, None, "Cache folder_id must be cleared after folder tombstone");
        }

        #[tokio::test]
        async fn apply_delta_skips_synced_suffix_folder() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            // Delta with a folder that has "(synced)" suffix -- should be skipped
            let delta = make_delta(
                vec![],
                vec![make_sync_folder("synced-artifact", "Work (synced)", 0, "2024-01-15T00:00:00Z")],
                vec![],
            );

            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            let exists: Option<String> = sqlx::query_scalar(
                "SELECT uuid FROM folders WHERE uuid = 'synced-artifact'"
            ).fetch_optional(&db.pool).await.unwrap();

            assert!(exists.is_none(), "Folder with '(synced)' suffix should be skipped");
            assert_eq!(report.pulled_folders, 0);
        }

        #[tokio::test]
        async fn apply_delta_cleans_up_existing_synced_folder_with_original() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            // Insert original "Work" folder and a "(synced)" artifact
            let orig_id = insert_folder_full(&db, "orig-work", "Work", 0, "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;
            let synced_id = insert_folder_full(&db, "synced-work", "Work (synced)", 1, "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;

            // Put a clip in the "(synced)" folder
            insert_clip_full(&db, "clip-in-synced", "text in synced", "hash-sync", Some(synced_id), "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;

            // Apply an empty delta -- the cleanup runs after delta application
            let delta = make_delta(vec![], vec![], vec![]);

            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            // "(synced)" folder should be deleted
            let synced_exists: Option<String> = sqlx::query_scalar(
                "SELECT name FROM folders WHERE name = 'Work (synced)'"
            ).fetch_optional(&db.pool).await.unwrap();
            assert!(synced_exists.is_none(), "'Work (synced)' folder should be merged/deleted");

            // Clip should have been moved to the original "Work" folder
            let clip_folder_id: Option<i64> = sqlx::query_scalar(
                "SELECT folder_id FROM clips WHERE uuid = 'clip-in-synced'"
            ).fetch_optional(&db.pool).await.unwrap().unwrap();
            assert_eq!(clip_folder_id, Some(orig_id), "Clip should be moved to original folder");

            assert_eq!(report.deleted, 1);
        }

        #[tokio::test]
        async fn apply_delta_renames_synced_folder_when_no_original() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            // Insert only a "(synced)" folder with no matching original
            insert_folder_full(&db, "orphan-synced", "Projects (synced)", 0, "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;

            let delta = make_delta(vec![], vec![], vec![]);
            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            // Should be renamed to "Projects" (without suffix)
            let renamed: Option<String> = sqlx::query_scalar(
                "SELECT name FROM folders WHERE uuid = 'orphan-synced'"
            ).fetch_optional(&db.pool).await.unwrap();
            assert_eq!(renamed, Some("Projects".to_string()), "Orphan (synced) folder should be renamed");

            assert_eq!(report.deleted, 1);
        }

        #[tokio::test]
        async fn apply_delta_multiple_deltas_in_sequence() {
            let db = setup_test_db().await;
            let drive = fake_drive();

            // First delta: insert a clip and a folder
            let mut report1 = SyncReport::default();
            let delta1 = make_delta(
                vec![make_sync_clip("seq-clip-1", "first", "hash-seq-1", None, "2024-01-01T00:00:00Z")],
                vec![make_sync_folder("seq-folder-1", "SeqFolder", 0, "2024-01-01T00:00:00Z")],
                vec![],
            );
            apply_delta(&db, &delta1, false, &drive, &mut report1).await.unwrap();
            assert_eq!(report1.pulled_clips, 1);
            assert_eq!(report1.pulled_folders, 1);

            // Second delta: update the clip and add another clip
            let mut report2 = SyncReport::default();
            let delta2 = make_delta(
                vec![
                    make_sync_clip("seq-clip-1", "first updated", "hash-seq-1-upd", None, "2024-01-02T00:00:00Z"),
                    make_sync_clip("seq-clip-2", "second", "hash-seq-2", Some("seq-folder-1"), "2024-01-02T00:00:00Z"),
                ],
                vec![],
                vec![],
            );
            apply_delta(&db, &delta2, false, &drive, &mut report2).await.unwrap();
            assert_eq!(report2.pulled_clips, 2);

            // Verify final state
            let preview1: String = sqlx::query_scalar("SELECT text_preview FROM clips WHERE uuid = 'seq-clip-1'")
                .fetch_one(&db.pool).await.unwrap();
            assert_eq!(preview1, "first updated");

            let folder_id_of_clip2: Option<i64> = sqlx::query_scalar(
                "SELECT folder_id FROM clips WHERE uuid = 'seq-clip-2'"
            ).fetch_one(&db.pool).await.unwrap();
            assert!(folder_id_of_clip2.is_some(), "Second clip should be in the folder");

            let total_clips: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM clips")
                .fetch_one(&db.pool).await.unwrap();
            assert_eq!(total_clips, 2);
        }

        #[tokio::test]
        async fn apply_delta_empty_delta_no_changes() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            // Insert a clip first
            insert_clip_full(&db, "existing-clip", "pre-existing", "hash-exist", None, "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;

            let delta = make_delta(vec![], vec![], vec![]);
            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            // Nothing should change
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM clips").fetch_one(&db.pool).await.unwrap();
            assert_eq!(count, 1);
            assert_eq!(report.pulled_clips, 0);
            assert_eq!(report.pulled_folders, 0);
            assert_eq!(report.deleted, 0);
        }

        #[tokio::test]
        async fn apply_delta_tombstone_for_nonexistent_clip_is_noop() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            let delta = make_delta(
                vec![],
                vec![],
                vec![Tombstone {
                    uuid: "nonexistent-clip".to_string(),
                    entity_type: "clip".to_string(),
                    deleted_at: "2024-01-15T00:00:00Z".to_string(),
                }],
            );

            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();
            assert_eq!(report.deleted, 0, "Tombstone for nonexistent clip should be no-op");
        }

        #[tokio::test]
        async fn apply_delta_tombstone_for_nonexistent_folder_is_noop() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            let delta = make_delta(
                vec![],
                vec![],
                vec![Tombstone {
                    uuid: "nonexistent-folder".to_string(),
                    entity_type: "folder".to_string(),
                    deleted_at: "2024-01-15T00:00:00Z".to_string(),
                }],
            );

            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();
            assert_eq!(report.deleted, 0, "Tombstone for nonexistent folder should be no-op");
        }

        #[tokio::test]
        async fn apply_delta_clip_with_folder_uuid_resolves_folder_id() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            // Insert a folder first
            let folder_id = insert_folder_full(&db, "target-folder", "Target", 0, "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;

            // Insert a clip that references the folder by UUID
            let delta = make_delta(
                vec![make_sync_clip("clip-with-folder", "text in folder", "hash-cwf", Some("target-folder"), "2024-01-15T00:00:00Z")],
                vec![],
                vec![],
            );

            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            let resolved_folder_id: Option<i64> = sqlx::query_scalar(
                "SELECT folder_id FROM clips WHERE uuid = 'clip-with-folder'"
            ).fetch_one(&db.pool).await.unwrap();

            assert_eq!(resolved_folder_id, Some(folder_id), "Clip folder_id should resolve from folder UUID");
        }

        #[tokio::test]
        async fn apply_delta_clip_with_unknown_folder_uuid_sets_null() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            // Insert a clip referencing a folder UUID that does not exist
            let delta = make_delta(
                vec![make_sync_clip("clip-orphan-folder", "orphan text", "hash-orphan", Some("nonexistent-folder-uuid"), "2024-01-15T00:00:00Z")],
                vec![],
                vec![],
            );

            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            let folder_id: Option<Option<i64>> = sqlx::query_scalar(
                "SELECT folder_id FROM clips WHERE uuid = 'clip-orphan-folder'"
            ).fetch_optional(&db.pool).await.unwrap();

            assert!(folder_id.is_some(), "Clip should exist");
            assert_eq!(folder_id.unwrap(), None, "folder_id should be NULL when folder UUID is not found");
        }

        #[tokio::test]
        async fn apply_delta_paste_count_takes_max() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            // Insert local clip with paste_count = 10
            sqlx::query(
                "INSERT INTO clips (uuid, clip_type, content, text_preview, content_hash,
                        is_deleted, is_pinned, is_sensitive, paste_count, created_at, last_accessed, updated_at)
                 VALUES ('paste-count-clip', 'text', 'hello', 'hello', 'hash-pc', 0, 0, 0, 10, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')"
            ).execute(&db.pool).await.unwrap();

            // Remote has paste_count = 5, but newer timestamp
            let mut remote_clip = make_sync_clip("paste-count-clip", "hello", "hash-pc", None, "2024-01-02T00:00:00Z");
            remote_clip.paste_count = 5;

            let delta = make_delta(vec![remote_clip], vec![], vec![]);
            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            let paste_count: i64 = sqlx::query_scalar(
                "SELECT paste_count FROM clips WHERE uuid = 'paste-count-clip'"
            ).fetch_one(&db.pool).await.unwrap();

            assert_eq!(paste_count, 10, "paste_count should be MAX(local=10, remote=5) = 10");
        }

        #[tokio::test]
        async fn apply_delta_image_clip_skipped_when_sync_images_false() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            let mut image_clip = make_sync_clip("img-clip-1", "", "img-hash-1", None, "2024-01-15T00:00:00Z");
            image_clip.clip_type = "image".to_string();
            image_clip.text_content = None;

            let delta = make_delta(vec![image_clip], vec![], vec![]);
            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            let exists: Option<String> = sqlx::query_scalar(
                "SELECT uuid FROM clips WHERE uuid = 'img-clip-1'"
            ).fetch_optional(&db.pool).await.unwrap();

            assert!(exists.is_none(), "Image clip should be skipped when sync_images=false");
            assert_eq!(report.pulled_clips, 0);
        }

        #[tokio::test]
        async fn apply_delta_preserves_clip_pinned_and_note() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            let mut clip = make_sync_clip("pinned-noted-clip", "important", "hash-pn", None, "2024-01-15T00:00:00Z");
            clip.is_pinned = true;
            clip.note = Some("This is important".to_string());

            let delta = make_delta(vec![clip], vec![], vec![]);
            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            let (is_pinned, note): (bool, Option<String>) = sqlx::query_as(
                "SELECT is_pinned, note FROM clips WHERE uuid = 'pinned-noted-clip'"
            ).fetch_one(&db.pool).await.unwrap();

            assert!(is_pinned, "is_pinned should be preserved");
            assert_eq!(note, Some("This is important".to_string()), "note should be preserved");
        }

        // ──────────────────────────────────────────────
        // C. build_full_state tests
        // ──────────────────────────────────────────────

        #[tokio::test]
        async fn build_full_state_empty_db() {
            let db = setup_test_db().await;

            let state = build_full_state(&db, "test-device", true).await.unwrap();

            assert!(state.clips.is_empty(), "Empty DB should produce empty clips");
            assert!(state.folders.is_empty(), "Empty DB should produce empty folders");
            assert!(state.tombstones.is_empty(), "Empty DB should produce empty tombstones");
            assert_eq!(state.device_id, "test-device");
        }

        #[tokio::test]
        async fn build_full_state_includes_clips_and_folders() {
            let db = setup_test_db().await;

            // Insert clips and folders
            let folder_id = insert_folder_full(&db, "state-folder-1", "Work", 0, "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;
            insert_clip_full(&db, "state-clip-1", "hello world", "hash-sc1", None, "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;
            insert_clip_full(&db, "state-clip-2", "in folder", "hash-sc2", Some(folder_id), "2024-01-02T00:00:00Z", "2024-01-02T00:00:00Z").await;

            let state = build_full_state(&db, "test-device", true).await.unwrap();

            assert_eq!(state.clips.len(), 2, "Should include all clips");
            assert_eq!(state.folders.len(), 1, "Should include all folders");
            assert_eq!(state.folders[0].name, "Work");

            // Clip in folder should have folder_uuid populated
            let clip_in_folder = state.clips.iter().find(|c| c.uuid == "state-clip-2").unwrap();
            assert_eq!(clip_in_folder.folder_uuid, Some("state-folder-1".to_string()));

            // Clip not in folder should have folder_uuid = None
            let clip_no_folder = state.clips.iter().find(|c| c.uuid == "state-clip-1").unwrap();
            assert_eq!(clip_no_folder.folder_uuid, None);
        }

        #[tokio::test]
        async fn build_full_state_excludes_images_when_disabled() {
            let db = setup_test_db().await;

            // Insert a text clip and an image clip
            insert_clip_full(&db, "text-clip-for-state", "some text", "hash-text-state", None, "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;

            sqlx::query(
                "INSERT INTO clips (uuid, clip_type, content, text_preview, content_hash,
                        is_deleted, is_pinned, is_sensitive, paste_count, created_at, last_accessed, updated_at)
                 VALUES ('image-clip-for-state', 'image', 'img.png', '', 'hash-img-state', 0, 0, 0, 0, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')"
            ).execute(&db.pool).await.unwrap();

            let state = build_full_state(&db, "test-device", false).await.unwrap();

            assert_eq!(state.clips.len(), 1, "Should only include text clips when sync_images=false");
            assert_eq!(state.clips[0].uuid, "text-clip-for-state");
        }

        #[tokio::test]
        async fn build_full_state_includes_tombstones() {
            let db = setup_test_db().await;

            // Insert tombstones
            sqlx::query("INSERT INTO sync_tombstones (uuid, entity_type, deleted_at) VALUES ('tomb-1', 'clip', '2024-01-15T00:00:00Z')")
                .execute(&db.pool).await.unwrap();
            sqlx::query("INSERT INTO sync_tombstones (uuid, entity_type, deleted_at) VALUES ('tomb-2', 'folder', '2024-01-16T00:00:00Z')")
                .execute(&db.pool).await.unwrap();

            let state = build_full_state(&db, "test-device", true).await.unwrap();

            assert_eq!(state.tombstones.len(), 2, "Should include all tombstones");
            assert_eq!(state.tombstones[0].uuid, "tomb-1");
            assert_eq!(state.tombstones[1].uuid, "tomb-2");
        }

        #[tokio::test]
        async fn build_full_state_text_content_populated_for_text_clips() {
            let db = setup_test_db().await;

            insert_clip_full(&db, "content-clip", "full text content here", "hash-ftc", None, "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;

            let state = build_full_state(&db, "test-device", true).await.unwrap();

            assert_eq!(state.clips.len(), 1);
            assert_eq!(state.clips[0].text_content, Some("full text content here".to_string()),
                "text_content should contain the full text for text clips");
        }

        // ──────────────────────────────────────────────
        // D. SyncReport tracking
        // ──────────────────────────────────────────────

        #[tokio::test]
        async fn sync_report_counts_correct_after_mixed_delta() {
            let db = setup_test_db().await;
            let drive = fake_drive();
            let mut report = SyncReport::default();

            // Pre-insert a clip to be tombstoned
            insert_clip_full(&db, "to-delete-for-report", "delete me", "hash-del-rep", None, "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z").await;

            let delta = make_delta(
                vec![
                    make_sync_clip("report-clip-1", "new clip one", "hash-r1", None, "2024-01-15T00:00:00Z"),
                    make_sync_clip("report-clip-2", "new clip two", "hash-r2", None, "2024-01-15T00:00:00Z"),
                ],
                vec![
                    make_sync_folder("report-folder-1", "ReportFolder", 0, "2024-01-15T00:00:00Z"),
                ],
                vec![
                    Tombstone {
                        uuid: "to-delete-for-report".to_string(),
                        entity_type: "clip".to_string(),
                        deleted_at: "2024-01-15T00:00:00Z".to_string(),
                    },
                ],
            );

            apply_delta(&db, &delta, false, &drive, &mut report).await.unwrap();

            assert_eq!(report.pulled_clips, 2, "Should report 2 pulled clips");
            assert_eq!(report.pulled_folders, 1, "Should report 1 pulled folder");
            assert_eq!(report.deleted, 1, "Should report 1 deleted item");
            assert!(!report.skipped, "Should not be skipped");
            assert!(report.errors.is_empty(), "Should have no errors");
        }

        #[tokio::test]
        async fn sync_report_default_values() {
            let report = SyncReport::default();

            assert_eq!(report.pushed_clips, 0);
            assert_eq!(report.pushed_folders, 0);
            assert_eq!(report.pulled_clips, 0);
            assert_eq!(report.pulled_folders, 0);
            assert_eq!(report.deleted, 0);
            assert!(!report.skipped);
            assert!(report.errors.is_empty());
        }
    }
}
