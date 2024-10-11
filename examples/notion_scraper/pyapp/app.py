import asyncio
from typing import List

import notion_scraper


# notion_scraper.reader_load_data() takes a list of page ids and returns a list of strings, where each string is the text content of a page.
# it uses the notion_scraper rust library to do the crawling.
async def notion_to_documents() -> List[str]:
    starting_page_id = "<put your page id here>"
    texts: List[str] = await notion_scraper.reader_load_data(page_ids=[starting_page_id])
    return [t for t in texts]

if __name__ == "__main__":
    asyncio.run(notion_to_documents())
