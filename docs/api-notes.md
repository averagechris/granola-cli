# Granola API Notes

Sources:

- <https://docs.granola.ai/introduction>
- <https://docs.granola.ai/help-center/sharing/integrations/granola-api>
- <https://docs.granola.ai/api-reference/openapi.json>

## Access

- Business or Enterprise plan required.
- API keys are created in the Granola desktop app: **Settings → Connectors → API keys**.
- Keys can include Personal notes, Public notes, or both.
- Enterprise admins may restrict which scopes members can create.

## Authentication

Use bearer API keys:

```http
Authorization: Bearer grn_YOUR_API_KEY
```

The docs mention browser-based OAuth for Granola MCP, but the public API documentation does not expose a CLI OAuth flow.

## Endpoints

### List notes

`GET https://public-api.granola.ai/v1/notes`

Query parameters:

- `created_before`: date or date-time
- `created_after`: date or date-time
- `updated_after`: date or date-time
- `folder_id`: `fol_...`
- `cursor`: pagination cursor
- `page_size`: 1-30, default 10

Response: `notes`, `hasMore`, `cursor`.

### Get note

`GET https://public-api.granola.ai/v1/notes/{note_id}`

Path parameter:

- `note_id`: `not_...`

Query parameters:

- `include=transcript`

Response includes title, owner, timestamps, web URL, calendar event, attendees, folder membership, summary text, summary markdown, and optional transcript.

### List folders

`GET https://public-api.granola.ai/v1/folders`

Query parameters:

- `cursor`
- `page_size`: 1-30, default 10

Response: `folders`, `hasMore`, `cursor`.

## API behavior to encode

- Note IDs use `not_...`, not UUIDs.
- Folder IDs use `fol_...`.
- List/get note endpoints only return notes with generated AI summary and transcript; processing notes may be absent or 404.
- Transcript speaker shape differs between macOS and iOS. Preserve unknown future fields where practical.
- Rate limit: 25-request burst and 5 requests/second sustained. 429 responses should be retried with backoff when safe.
- No webhooks are currently available; polling is expected.
