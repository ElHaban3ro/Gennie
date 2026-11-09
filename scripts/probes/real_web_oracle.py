#!/usr/bin/env python3
"""Oráculo independiente para las dos páginas públicas, sin usar código de agentes."""
from html.parser import HTMLParser
from urllib.request import urlopen


class Parser(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.rows=[];self.row=None;self.depth=0;self.capture=None;self.parts=[]
    def handle_starttag(self,tag,attrs):
        attrs=dict(attrs);classes=attrs.get('class','').split()
        if tag=='div' and 'quote' in classes and self.row is None:
            self.row={'text':'','author':'','tags':[]};self.depth=1
        elif self.row is not None and tag=='div':
            self.depth+=1
        if self.row is not None and self.capture is None:
            kind='text' if tag=='span' and 'text' in classes else 'author' if tag=='small' and 'author' in classes else 'tag' if tag=='a' and 'tag' in classes else None
            if kind:self.capture=(tag,kind);self.parts=[]
    def handle_data(self,data):
        if self.capture:self.parts.append(data)
    def handle_endtag(self,tag):
        if self.capture and tag==self.capture[0]:
            value=''.join(self.parts).strip();kind=self.capture[1]
            if kind=='tag':self.row['tags'].append(value)
            else:self.row[kind]=value
            self.capture=None;self.parts=[]
        if self.row is not None and tag=='div':
            self.depth-=1
            if self.depth==0:
                self.rows.append(self.row);self.row=None


def read(proxy_port):
    rows=[]
    for page in (1,2):
        with urlopen(f'http://127.0.0.1:{proxy_port}/page/{page}',timeout=12) as response:
            data=response.read(1_000_001)
        if len(data)>1_000_000:raise ValueError('page_too_large')
        parser=Parser();parser.feed(data.decode('utf-8'));rows.extend(parser.rows)
    if len(rows)!=20 or any(not row['text'] or not row['author'] or not row['tags'] for row in rows):
        raise ValueError('unexpected_site_structure')
    return rows
