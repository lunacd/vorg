# Vorg

## What is vorg?

Vorg is a tag-based content manager. Different collections of files are organized into
repositories. Then files can be organized into collections and collections can be given a title and
many tags.

## What are the benefits?

- Never again have identical copies of the same file scattered across your file system.
- Fast full-text-searches on title and autocomplete-able searches based on tags.
- Group files on multiple dimensions using tags, rather than inventing some complex directory
  structure.

## Database schemas

### Current (V2)

```sql
CREATE TABLE tags (
    tag_id  INTEGER NOT NULL,
    name    TEXT NOT NULL,
    PRIMARY KEY("tag_id")
);
CREATE TABLE collections (
    collection_id   INTEGER NOT NULL,
    title           TEXT NOT NULL,
    PRIMARY KEY("collection_id")
);
CREATE TABLE collection_tag (
    collection_id   INTEGER NOT NULL,
    tag_id          INTEGER NOT NULL,
    PRIMARY KEY("collection_id","tag_id"),
    FOREIGN KEY("tag_id") REFERENCES "tags"("tag_id"),
    FOREIGN KEY("collection_id") REFERENCES "collections"("collection_id")
);
CREATE TABLE items (
    item_id         INTEGER NOT NULL,
    collection_id   INTEGER NOT NULL,
    ext             TEXT NOT NULL,
    hash            VARCHAR(64) NOT NULL,
    PRIMARY KEY("item_id"),
    FOREIGN KEY("collection_id") REFERENCES "collections"("collection_id")
);
CREATE TABLE collection_item (
    collection_id   INTEGER NOT NULL,
    item_id         INTEGER NOT NULL,
    PRIMARY KEY("collection_id","item_id"),
    FOREIGN KEY("collection_id") REFERENCES "collections"("collection_id"),
    FOREIGN KEY("item_id") REFERENCES "items"("item_id")
);
CREATE VIRTUAL TABLE title_fts USING fts5 (
    title,
    content='collections',
    content_rowid='collection_id'
);
CREATE UNIQUE INDEX hash_index ON items (
    hash
);
CREATE UNIQUE INDEX tag_index ON tags (
    name
);
CREATE TRIGGER title_insert AFTER INSERT ON collections BEGIN
    INSERT INTO title_fts(rowid, title) VALUES (new.collection_id, new.title);
END;
CREATE TRIGGER title_delete AFTER DELETE ON collections BEGIN
    INSERT INTO title_fts(title_fts, rowid, title)
        VALUES('delete', old.collection_id, old.title);
END;
CREATE TRIGGER title_update AFTER UPDATE ON collections BEGIN
    INSERT INTO title_fts(fts_idx, rowid, title) VALUES('delete', old.collection_id, old.title);
    INSERT INTO title_fts(rowid, title) VALUES (new.collection_id, new.title);
END;
```

### V1

```sql
CREATE TABLE IF NOT EXISTS "actors" (
    "actor_id"    INTEGER NOT NULL,
    "name"    TEXT NOT NULL,
    PRIMARY KEY("actor_id")
);
CREATE TABLE IF NOT EXISTS "studios" (
    "studio_id"    INTEGER NOT NULL,
    "name"    TEXT NOT NULL,
    PRIMARY KEY("studio_id")
);
CREATE TABLE IF NOT EXISTS "tags" (
    "tag_id"    INTEGER NOT NULL,
    "name"    TEXT NOT NULL,
    PRIMARY KEY("tag_id")
);
CREATE TABLE IF NOT EXISTS "items" (
    "item_id"    INTEGER NOT NULL,
    "hash"    VARCHAR(64) NOT NULL,
    "title"    TEXT NOT NULL,
    "ext"    TEXT NOT NULL,
    "studio_id"    INTEGER NOT NULL,
    PRIMARY KEY("item_id"),
    FOREIGN KEY("studio_id") REFERENCES "studios"("studio_id")
);
CREATE TABLE IF NOT EXISTS "item_actor" (
    "item_id"    INTEGER NOT NULL,
    "actor_id"    INTEGER NOT NULL,
    PRIMARY KEY("item_id","actor_id"),
    FOREIGN KEY("actor_id") REFERENCES "actors"("actor_id"),
    FOREIGN KEY("item_id") REFERENCES "items"("item_id")
);
CREATE TABLE IF NOT EXISTS "item_tag" (
    "item_id"    INTEGER NOT NULL,
    "tag_id"    INTEGER NOT NULL,
    PRIMARY KEY("item_id","tag_id"),
    FOREIGN KEY("item_id") REFERENCES "items"("item_id"),
    FOREIGN KEY("tag_id") REFERENCES "tags"("tag_id")
);
CREATE UNIQUE INDEX IF NOT EXISTS "hash_index" ON "items" (
    "hash"
);
CREATE UNIQUE INDEX IF NOT EXISTS "actor_index" ON "actors" (
    "name"
);
CREATE UNIQUE INDEX IF NOT EXISTS "tag_index" ON "tags" (
    "name"
);
CREATE UNIQUE INDEX IF NOT EXISTS "studio_index" ON "studios" (
    "name"
);
```

## FAQs

- Why is there mentions of actors and studios throughout the codebase?

Currently vorg is specifically geared towards cataloguing video and audio files, with studio and
actors being the two builtin metadata types. In a later version, vorg plan to support custom types
on a per-repository basis.

More concretely, studio information is stored as a "studio:[studio name]" tags and generic tags are
stored as "tag:[tag name]". Later versions will support customizing what types of tags a repository
will be able to support. For example, a repo for business data can have "department" and "urgency"
as builtin tags, whereas a repo for photos can have "location" and "occasion" as builtin tags.
