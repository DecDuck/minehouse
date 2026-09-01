-- item stacks are per-container


create table item_stack(
    id uuid primary key default gen_random_uuid(),
    container_id uuid not null references container(id) on delete cascade,

    item_kind text not null,
    -- slot inside the container
    slot int not null,
    -- Data-component patch (custom name, enchantments, durability, NBT...).
    -- '{}' == plain fungible item; non-empty == a distinct/complex item.
    components jsonb not null default '{}'::jsonb,
    quantity int not null check (quantity > 0),
    -- Fingerprint of the stack identity: two stacks are the same item iff
    -- item_kind + digest match. Lets `components` take part in the unique key.
    components_digest text generated always as (md5(components::text)) stored,

    unique (container_id, slot)
);

create index on item_stack (item_kind);

create index on item_stack (container_id);

create index on item_stack using gin (components);

CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX IF NOT EXISTS item_stack_kind_search ON item_stack USING gin (
    lower(
        replace (item_kind, '_', ' ')
    ) gin_trgm_ops
);