# MI4ULINGS DOCLING - Work Log

This file tracks the implementation progress of the [DOCLING.md](./DOCLING.md) plan.

## v0.2 Changes

*08.04.2025 05:18*

[REF](./DOCLING.md#v02) - BUG: .md files are non converted html files, they should be converted (CHANGE [] -> [x])

- SUMMARY: 
    - Fixed HTML to Markdown conversion in converter.rs

- DESCRIPTION: 
    - Implemented proper HTML to Markdown conversion in the converter module
    - Added support for extracting document titles to use as headers
    - Improved fallback converter implementation
    - Added robust error handling for conversion failures
    - Ensured all three conversion methods (htmd, fast_html2md, jina_reader) work properly

- REASONING: 
    - The previous implementation was just returning HTML with a simple header prefix
    - Real conversion was not happening, which resulted in .md files that were actually HTML files
    - Implemented proper converters to ensure actual HTML to Markdown transformation

- STATUS: success

*08.04.2025 05:22*

[REF](./DOCLING.md#v02) - BUG: .md files should be stripped from any css, js, html tags, images..etc (CHANGE [] -> [x])

- SUMMARY: 
    - Enhanced HTML tag removal and cleanup in processor.rs

- DESCRIPTION: 
    - Improved the processor clean_content method to strip all HTML tags, not just media-related ones
    - Added logic to skip CSS and JavaScript content blocks
    - Implemented better text extraction from HTML elements with attributes
    - Improved handling of HTML structure to extract only text content
    - Added support for using document title as header in Markdown

- REASONING: 
    - Previous implementation only removed img, video, and audio tags
    - Full HTML tags, CSS, and JS were still present in the Markdown files
    - Better cleanup ensures real Markdown output without HTML markup

- STATUS: success

*08.04.2025 05:29*

[REF](./DOCLING.md#v02) - BUG: list parameter does not show deep parameter nor retry/try count (CHANGE [] -> [x])

- SUMMARY: 
    - Enhanced list command to show depth and retry information

- DESCRIPTION: 
    - Updated the list command output in main.rs
    - Added display of crawl depth (how deep crawler goes)
    - Added display of try count and maximum retry count
    - Improved URL and download time formatting
    - Made list output more comprehensive with all entry parameters

- REASONING: 
    - Previous implementation only showed NAME, URL, STATUS, and LAST DOWNLOAD
    - Users need to see crawl depth and retry information for better management
    - Complete information helps users understand and manage entries better

- STATUS: success

*08.04.2025 05:34*

[REF](./DOCLING.md#v02) - FEATURE: success converted result should be copied to root folder of workspace to dir docs/docling_output (CHANGE [] -> [x])

- SUMMARY: 
    - Implemented automatic copying of successful results to docs/docling_output

- DESCRIPTION: 
    - Modified the run_entry function in lib.rs
    - Added code to create the docs/docling_output directory if it doesn't exist
    - Implemented copying of successful results to this directory after processing
    - Added proper error handling for failed copies
    - Added logging for successful copies

- REASONING: 
    - Having results in a central location makes them easier to access
    - This improves user workflow by removing the need to manually copy results

- STATUS: success

*08.04.2025 05:38*

[REF](./DOCLING.md#v02) - FEATURE: add docstring to document code to existing codebase (CHANGE [] -> [x])

- SUMMARY: 
    - Added comprehensive docstrings to document the codebase

- DESCRIPTION: 
    - Added detailed module-level documentation to all source files
    - Added function-level documentation with parameters, return values, and error cases
    - Added struct and field documentation
    - Added examples and usage notes where appropriate
    - Improved code readability with more descriptive comments

- REASONING: 
    - Documentation improves code maintainability
    - Makes the codebase more accessible to new developers
    - Helps users understand the design and functionality

- STATUS: success

## v0.3 Changes

*08.04.2025 05:42*

[REF](./DOCLING.md#v03) - BUG: creating entry without deep parameter always sets deep=1 instead of default_deep (CHANGE [] -> [x])

- SUMMARY: 
    - Fixed default_deep value usage in UrlEntry::new

- DESCRIPTION: 
    - Modified UrlEntry::new method to use config.data.default_deep when crawl_depth is None
    - Updated add_url function to store depth before entry is moved
    - Fixed related error handling and parameter passing
    - Ensured new entries respect the configured default_deep value

- REASONING: 
    - Previous implementation always used a hardcoded DEFAULT_CRAWL_DEPTH value
    - Proper use of user-configured default_deep makes the program more flexible

- STATUS: success

*08.04.2025 05:46*

[REF](./DOCLING.md#v03) - BUG: hanging issue during downloading (CHANGE [] -> [x])

- SUMMARY: 
    - Fixed hanging issue by implementing proper async crawling with tokio

- DESCRIPTION: 
    - Completely redesigned the crawler implementation using tokio concurrency primitives
    - Implemented AsyncSpider class with broadcast channels for proper task communication
    - Added semaphores to enforce concurrent request limits
    - Implemented proper worker task supervision and coordination
    - Fixed deadlock issues with proper scoping of locks and references

- REASONING: 
    - Previous implementation was not properly handling async tasks
    - The Spider class wasn't properly using tokio for concurrent processing
    - New implementation leverages tokio broadcast/spawn for truly asynchronous operation

- STATUS: success

*08.04.2025 05:49*

[REF](./DOCLING.md#v03) - BUG: media files filtering incorrectly (CHANGE [] -> [x])

- SUMMARY: 
    - Fixed media filtering to only download images

- DESCRIPTION: 
    - Enhanced the download_images function to only process image files
    - Added Content-Type header checking to verify files are actually images
    - Improved image URL extraction to handle all src attribute formats
    - Added proper error handling for failed image downloads
    - Implemented better filtering to avoid downloading CSS, JS, and other non-image files

- REASONING: 
    - Previous implementation downloaded all linked media files including CSS and JS
    - Images-only approach reduces unnecessary downloads and storage use
    - Content-Type verification ensures only actual images are saved

- STATUS: success