import urllib.request
from bs4 import BeautifulSoup
import re
from datetime import datetime

url = "https://en.wikipedia.org/wiki/2026_FIFA_World_Cup"
req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})
html = urllib.request.urlopen(req).read()
soup = BeautifulSoup(html, 'html.parser')

matches = []
for box in soup.find_all('div', class_='footballbox'):
    try:
        date_th = box.find('th', class_='fdate')
        if not date_th: continue
        date_str = date_th.get_text(strip=True)
        
        home = box.find('th', class_='fhome').get_text(strip=True)
        score = box.find('th', class_='fscore').get_text(strip=True)
        away = box.find('th', class_='faway').get_text(strip=True)
        
        if score and "v" not in score.lower():
            matches.append(f"{date_str}: {home} {score} {away}")
    except Exception as e:
        pass

print("Recent Matches:")
for m in matches[-15:]:
    print(m)
