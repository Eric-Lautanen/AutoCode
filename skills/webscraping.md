---
name: webscraping
description: Use when extracting data from websites - fetching HTML, parsing structure, handling pagination, dealing with JavaScript-rendered content, and respecting rate limits. Load when a task involves scraping data from a website or automating web data extraction.
---

# Web Scraping

## Overview

Web scraping is extracting structured data from web pages. The core principle: **try the simplest approach first (HTTP GET + parse), escalate to headless browsers only when necessary.** A headless browser is 100x slower and more complex than a simple HTTP request. Don't use one unless the page requires JavaScript to render.

## Fetch First

### Try HTTP GET Before a Headless Browser
```python
import requests
from bs4 import BeautifulSoup

response = requests.get("https://example.com/products")
soup = BeautifulSoup(response.text, "html.parser")
products = soup.select(".product-card")
```

**When this works:** Most server-rendered pages, APIs disguised as HTML, static sites.

**When you need a headless browser:**
- Content loads via JavaScript after the initial page load
- You need to interact with the page (click, scroll, fill forms)
- The site detects and blocks simple HTTP requests

## HTML Parsing

### CSS Selectors vs. XPath
| Approach | Pros | Cons |
|----------|------|------|
| CSS selectors | Familiar, readable, fast | Can't select by text content, limited parent traversal |
| XPath | Powerful, text selection, parent traversal | More complex syntax, less familiar |

**Prefer CSS selectors** for most tasks. Use XPath only when you need to select by text content or traverse upward.

### Finding Stable Selectors
```python
# BAD — positional, fragile
soup.select("div > div > div > span")[2]

# GOOD — semantic class or attribute
soup.select("[data-product-id]")
soup.select(".product-name")
soup.select("article.product h2")
```

**Rules for stable selectors:**
- Prefer `data-*` attributes (designed for this purpose)
- Use semantic class names over structural position
- Avoid deeply nested selectors — they break when layout changes
- Test against multiple pages to confirm the selector is consistent

## Pagination

### Next-Page Links
```python
url = "https://example.com/products"
while url:
    response = requests.get(url)
    soup = BeautifulSoup(response.text)
    # ... extract data ...
    next_link = soup.select_one("a.next-page")
    url = next_link["href"] if next_link else None
```

### Offset Parameters
```python
for offset in range(0, 10000, 50):
    response = requests.get(f"https://example.com/api/items?offset={offset}&limit=50")
    data = response.json()
    if not data["items"]:
        break
    process(data["items"])
```

### Infinite Scroll Detection
- Look for API calls in the Network tab (often returns JSON — scrape the API instead!)
- Check for `data-next-page` or similar attributes
- If the site uses cursor-based pagination, follow the cursors

## JavaScript-Rendered Content

### When You Need a Headless Browser
- The page shows "Loading..." and content appears after JavaScript runs
- Network tab shows XHR/fetch calls that load the data (try hitting those APIs directly first!)
- You need to log in or interact with the page

### Playwright (Recommended)
```python
from playwright.sync_api import sync_playwright

with sync_playwright() as p:
    browser = p.chromium.launch()
    page = browser.new_page()
    page.goto("https://example.com/products")
    page.wait_for_selector(".product-card")
    products = page.query_selector_all(".product-card")
    for product in products:
        name = product.query_selector(".name").inner_text()
        price = product.query_selector(".price").inner_text()
    browser.close()
```

**Always wait for content:** `page.wait_for_selector(".product-card")` — don't assume the page is loaded just because `goto()` returned.

## Rate Limiting

### Delay Between Requests
```python
import time, random

for url in urls:
    response = requests.get(url)
    process(response)
    time.sleep(random.uniform(1, 3))  # 1-3 second delay with jitter
```

### Respect robots.txt
```python
import urllib.robotparser

rp = urllib.robotparser.RobotFileParser()
rp.set_url("https://example.com/robots.txt")
rp.read()

if rp.can_fetch("MyBot/1.0", "https://example.com/products"):
    scrape("https://example.com/products")
```

### Rotate User Agents Carefully
```python
USER_AGENTS = [
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
]
headers = {"User-Agent": random.choice(USER_AGENTS)}
```

**Note:** Rotating user agents to evade detection is ethically questionable and may violate terms of service. Use responsibly.

## Session and Cookies

### Logging In
```python
session = requests.Session()

# Submit login form
session.post("https://example.com/login", data={
    "username": "user",
    "password": "pass",
})

# Subsequent requests use the session cookies
response = session.get("https://example.com/dashboard")
```

## Data Extraction

### Target What You Need
```python
# BAD — parse everything, store everything
all_html = soup.prettify()

# GOOD — extract only the data you need
products = []
for card in soup.select(".product-card"):
    products.append({
        "name": card.select_one(".name").text.strip(),
        "price": card.select_one(".price").text.strip(),
        "url": card.select_one("a")["href"],
    })
```

## Fragility

### Build In Error Detection
```python
def scrape_products(url):
    response = requests.get(url)
    soup = BeautifulSoup(response.text)
    products = soup.select(".product-card")
    
    if not products:
        # Expected structure not found — site may have changed
        raise ScrapingError(f"No products found at {url}. Selector may be stale.")
    
    return [extract_product(p) for p in products]
```

**Monitoring:**
- Track the number of items extracted per page
- Alert when counts drop significantly (site may have changed)
- Log pages that fail to parse for manual review

## Anti-Patterns

- **Using a headless browser when a simple GET works.** 100x slower and more complex.
- **Positional selectors.** `div > div > div > span` breaks on any layout change.
- **No rate limiting.** You'll get IP-banned and potentially take down a small site.
- **Not checking robots.txt.** Respect the site's crawling preferences.
- **Scraping everything.** Extract only the data you need.
- **No error detection.** A scraper that silently returns empty results is worse than one that fails loudly.
