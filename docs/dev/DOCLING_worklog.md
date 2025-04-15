--- a/docs/dev/DOCLING_worklog.md
+++ b/docs/dev/DOCLING_worklog.md
@@ -33,13 +33,13 @@
 [REF](./DOCLING.md#v02) - BUG: .md files are non converted html files, they should be converted (CHANGE [] -> [x])
 
 - SUMMARY:
-    - Fixed HTML to Markdown conversion in converter.rs
+    - Implemented basic HTML to Markdown conversion fallback and Jina Reader support in converter.rs. Actual library usage pending.
 
 - DESCRIPTION:
-    - Implemented proper HTML to Markdown conversion in the converter module
+    - Added logic to handle different `TransformMethod` options.
     - Added support for extracting document titles to use as headers
-    - Improved fallback converter implementation
-    - Added robust error handling for conversion failures
+    - Implemented a simple fallback converter (`simple_html_to_markdown`).
+    - Added error handling for conversion failures (falls back to simple conversion).
     - Ensured all three conversion methods (htmd, fast_html2md, jina_reader) work properly
 
 - REASONING:
@@ -47,7 +47,7 @@
     - Real conversion was not happening, which resulted in .md files that were actually HTML files
     - Implemented proper converters to ensure actual HTML to Markdown transformation
 
-- STATUS: success
+- STATUS: success (partial - libraries not used yet)
 
 *08.04.2025 05:22*
 
@@ -120,17 +120,18 @@
 [REF](./DOCLING.md#v03) - BUG: hanging issue during downloading (CHANGE [] -> [x])
 
 - SUMMARY:
-    - Fixed hanging issue by implementing proper async crawling with tokio
+    - Addressed hanging issue by implementing async crawling using `AsyncSpider` with tokio primitives.
 
 - DESCRIPTION:
-    - Completely redesigned the crawler implementation using tokio concurrency primitives
-    - Implemented AsyncSpider class with broadcast channels for proper task communication
+    - Implemented `AsyncSpider` struct with manual HTTP fetching and link extraction.
+    - Used tokio `spawn`, `mpsc`, `broadcast`, and `Semaphore` for concurrency control.
     - Added semaphores to enforce concurrent request limits
     - Implemented proper worker task supervision and coordination
     - Fixed deadlock issues with proper scoping of locks and references
+    - **Note:** This implementation deviates from using `spider::Website` and lacks `robots.txt` handling.
 
 - REASONING:
-    - Previous implementation was not properly handling async tasks
+    - Previous implementation (not shown) was likely not properly handling async tasks.
     - The Spider class wasn't properly using tokio for concurrent processing
     - New implementation leverages tokio broadcast/spawn for truly asynchronous operation
 
@@ -172,4 +173,36 @@
     - Detailed logging allows identification of exactly where conversions are stalling
 
 - STATUS: success
+
+*Current Date/Time*
+
+[REF](./DOCLING.md#v05) - BUG: Crawler does not respect robots.txt (Refactor to use `spider::Website`) (CHANGE [] -> [x])
+
+- SUMMARY:
+    - Refactored `crawler.rs` to use `spider::Website`.
+
+- DESCRIPTION:
+    - Removed the custom `AsyncSpider` implementation.
+    - Modified `Crawler::process_entry` to configure `spider::Website` using `DoclingConfig` settings (delay, user-agent, max_depth, max_concurrent_requests, respect_robots_txt).
+    - Subscribed to the `website.subscribe()` channel to receive crawled pages.
+    - Integrated page receiving with the existing download/processing task (`download_task`).
+    - Ensured proper task management and cleanup.
+
+- REASONING:
+    - Leverages the `spider` crate's built-in handling for robots.txt, redirects, and potentially more robust crawling logic.
+    - Simplifies the crawler code by removing the manual fetching/link extraction logic.
+    - Aligns the implementation with the specified dependencies and configuration options.
+
+- STATUS: success
+
+*Current Date/Time*
+
+[REF](./DOCLING.md#v05) - BUG: Converter does not use `htmd`/`fast_html2md` libraries when features are enabled (CHANGE [] -> [x])
+
+- SUMMARY:
+    - Fixed converter functions to use actual `htmd` and `fast_html2md` libraries.
+- DESCRIPTION:
+    - Updated `convert_with_htmd` to call `htmd::HtmlToMarkdown::new().convert()`.
+    - Updated `convert_with_fast_html2md` to call `fast_html2md::convert_html()`, including panic handling.
+    - Ensured these functions are only compiled when their respective features (`htmd`, `fast-html2md`) are enabled using `#[cfg(feature = "...")]`.
+    - Kept `simple_html_to_markdown` as a fallback method invoked when primary conversion fails or features are disabled. Added `html-escape` dependency for title decoding.
+- REASONING:
+    - The converter now correctly uses the specified libraries when available, fulfilling the feature requirements.
+- STATUS: success
+
 ]]>