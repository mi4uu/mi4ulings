--- a/docs/dev/DOCLING.md
+++ b/docs/dev/DOCLING.md
@@ -11,11 +11,11 @@
     - remove [name]
     - start [name]
 
-- when element is started, program should use spider library and tokio, and try to gather all links in provided url, to the given depth,
+- when element is started, program should use the `spider` library and tokio, and try to gather all links in provided url, to the given depth,
 links need to starts with provided url.
 - next it will go thru all of the links, download pages (respecting config settings)
 . each of them will save each of pages into outputs_path/NAME/output_parts_html_suffix/name_of_page.html
-and media files if any will be saved into  outputs_path/NAME/output_parts_media_suffix/
+and image files (based on Content-Type) if any will be saved into outputs_path/NAME/output_parts_media_suffix/
 
 - next convert add html files to markdown using htmd or fast_html2md or jina_reader depending on config. if jina_reader is used it not using any extra crates, just prefix any url with https://r.jina.ai/ when downloading it content, than there is no conversation needed.
 
@@ -52,11 +52,11 @@
 
 ## v.0.2 (Historical - See DOCLING_worklog.md for details)
 [x] BUG: .md files are non converted html files, they should be converted
-[x] BUG: .md files should be stripped from any css, js, html tags, images..etc and should benerated only from body of the document, title should be used before this content as header
+[x] BUG: .md files should be stripped from any css, js, html tags, images..etc and should generated only from body of the document, title should be used before this content as header
 [x] BUG: features in cargo.toml was badly writen end it was resulting in error without fast_hrml2md - fixed
-[] implement comprehensive unit tests
-[] implement integration tests
-[] implement e2e tests
+[-] implement comprehensive unit tests
+[-] implement integration tests
+[-] implement e2e tests
 [x] BUG: list parameter does not show deep parameter nor retry/try count
 [x] FEATURE: success converted result should be copied to root folder of workspace to dir docs/docling_output
 [x] FEATURE: add docstring to document code to existing codebase
@@ -64,11 +64,14 @@
 ## v0.3 (Historical - See DOCLING_worklog.md for details)
 [x] BUG: creating entry without deep parameter always sets deep=1 instead of default_deep
 [x] BUG: hanging issue during downloading - fixed using tokio broadcast/spawn for concurrent processing
-[x] BUG: media files filtering incorrectly - now only image files are downloaded (verified by Content-Type header)
+[x] BUG: media files filtering incorrectly - now only image files are downloaded (verified by Content-Type header) - Implemented in `crawler.rs`.
 
 ## v0.4 (Historical - See DOCLING_worklog.md for details)
 [x] BUG: url entries not converting but waiting indefinitely - implement verbose logging to track conversion progress
 
+## v0.5 (Current)
+[ ] BUG: Crawler does not respect robots.txt (Refactor to use `spider::Website`)
+[ ] BUG: Converter does not use `htmd`/`fast_html2md` libraries when features are enabled
+
 