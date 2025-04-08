# NAME
mi4ulings-docling

# LOCATION
crates/docling

# DESCRIPTION
store in toml file list of urls to download with
last download date and time, last try date and time, last fail date and time, how deep it should crawl, 
status (enabled, disabled, failed - halted), version and NAME
it has bin that take actions:
    - add [url]
    - stop [name]
    - list 
    - remove [name]
    - start [name]

- when element is started, program should use spider library and tokio, and try to gather all links in provided url, to the given depth,
links need to starts with provided url.
- next it will go thru all of the links, download pages (respecting config settings)
. each of them will save each of pages into outputs_path/NAME/output_parts_html_suffix/name_of_page.html
and media files if any will be saved into  outputs_path/NAME/output_parts_media_suffix/

- next convert add html files to markdown using htmd or fast_html2md or jina_reader depending on config. if jina_reader is used it not using any extra crates, just prefix any url with https://r.jina.ai/ when downloading it content, than there is no conversation needed.

- next step is to combine all of output markdown files into one big markdown file, and remove all of images, media, multiple white characters and links that are not from oryginal given domain.

- next file can be saved as an result in outputs_path/NAME/output_parts_markdown_results_suffix/proper-name.md  and it is a success.
- in case of fail on any steps error should be saved into outputs_path/NAME/ERRORS/ with detailed information, time trace, and any information that might give a hint, than if there are still retry left , wait and try again.
.

- 

# STACK
- spider
- tokio
- htmd (behind feature, included in default)
- fast_html2md (behind feature)


# CONFIG
-  inputs_path 
-  outputs_path
-  output_parts_html_suffix (default=parts_html)
- output_parts_media_suffix (default=parts_media)
- output_parts_markdown_suffix (default=parts_md)
- output_parts_markdown_results_suffix (default=results_md)
-  retry_count (default 3)
- delay_between_request_in_ms (default=500)
- max_concurrent_requests (default=1)
- user_agent (default to mi4uling-docling-bot)
- respect_robots_txt (default true)
- transform_md_using ( one of fast_html2md, htmd, jina_reader ) htmd by default
- retry_delay (array of retry_count elements in seconds - default: 10, 40, 200)
- refetch after days (default 100)
- default deep


# PLAN

## v.0.1

[x] plan step by step program execution with details
[x] extending this plan with details, and fix mistakes
[x] create config implementation
[-] create unit tests for planed program (basic tests implemented, comprehensive tests pending)
[x] implement program
[-] test program (manual testing required)

## v0.2

[x] BUG: .md files are non converted html files, they should be converted
[x] BUG: .md files should be stripped from any css, js, html tags, images..etc and should benerated only from body of the document, title should be used before this content as header
[x] BUG: features in cargo.toml was badly writen end it was resulting in error without fast_hrml2md - fixed
[] implement comprehensive unit tests
[] implement integration tests
[] implement e2e tests
[x] BUG: list parameter does not show deep parameter nor retry/try count
[x] FEATURE: success converted result should be copied to root folder of workspace to dir docs/docling_output
[x] FEATURE: add docstring to document code to existing codebase

## v0.3

[x] BUG: creating entry without deep parameter always sets deep=1 instead of default_deep
[x] BUG: hanging issue during downloading - fixed using tokio broadcast/spawn for concurrent processing
[x] BUG: media files filtering incorrectly - now only image files are downloaded (verified by Content-Type header)
