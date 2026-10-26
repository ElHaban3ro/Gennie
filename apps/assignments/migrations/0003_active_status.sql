UPDATE assignments
SET document = json_set(document, '$.status', 'active')
WHERE json_extract(document, '$.status') = 'recorded';
