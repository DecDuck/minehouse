CREATE EXTENSION IF NOT EXISTS cube;

create type region_type as enum ('bulk', 'pickface', 'putaway', 'processing', 'order');

create table container_region (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid (),
    type region_type not null,
    world_region cube not null,
    -- Reject regions whose cubes overlap (cube `&&`), incl. shared boundaries
    exclude using gist (
        world_region
        with
            &&
    )
);

create table container (
    id uuid primary key default gen_random_uuid (),
    region_id uuid not null,
    constraint pk_region_id foreign key (region_id) references container_region (id) on delete cascade,
    position cube not null,
    -- Number of inventory slots this container block exposes
    capacity integer not null check (capacity > 0)
);

create type crafting_engine_type as enum ('crafting_table', 'smithing', 'stonecutter', '');

create table crafting_engine (
    id uuid primary key default gen_random_uuid (),
    role region_type not null,
    position cube not null
);

create table generic_crafting_engine (
    id uuid primary key default gen_random_uuid (),
    role region_type not null,
    in_position cube not null,
    out_position cube not null
)