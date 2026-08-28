-- Bulk and order-pickface containers are each pinned to a single item category.
create type item_category as enum ('rocks', 'wood', 'tools', 'materials', 'food', 'mob_drops', 'other');

-- Null for containers that are not category-pinned (e.g. putaway, user pickface).
alter table container add column category item_category;

create index on container (category);
