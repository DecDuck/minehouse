delete from craft_job;

create type craft_node_kind as enum ('recipe', 'storage', 'any');
create type craft_node_state as enum (
    'pending', 'blocked', 'staging', 'queued', 'running', 'completed',
    'retry_wait', 'failed', 'cancelled'
);
create type craft_operation_kind as enum (
    'stage_transfer', 'dependency_transfer', 'craft', 'cleanup_transfer'
);
create type craft_operation_state as enum (
    'queued', 'claimed', 'running', 'completed', 'failed', 'cancelled', 'interrupted'
);

alter table crafting_engine
    add column region_id uuid not null references container_region(id) on delete cascade,
    add column engine_type crafting_engine_type not null default 'crafting_table';

create unique index crafting_engine_region_id_idx on crafting_engine (region_id);
create index crafting_engine_type_idx on crafting_engine (engine_type);

create table craft_job_node (
    id uuid primary key,
    job_id uuid not null references craft_job(id) on delete cascade,
    parent_id uuid references craft_job_node(id) on delete cascade,
    key text not null,
    child_order integer not null check (child_order >= 0),
    kind craft_node_kind not null,
    item_kind text not null,
    required_quantity integer not null check (required_quantity > 0),
    recipe_id uuid,
    engine_type crafting_engine_type,
    recipe_ingredients jsonb,
    planned_crafts integer check (planned_crafts > 0),
    output_yield integer check (output_yield > 0),
    completed_crafts integer not null default 0 check (completed_crafts >= 0),
    completed_quantity integer not null default 0 check (completed_quantity >= 0),
    state craft_node_state not null default 'pending',
    assigned_region_id uuid references container_region(id),
    retry_count integer not null default 0 check (retry_count >= 0),
    retry_at timestamptz,
    error text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (job_id, key),
    check (completed_quantity <= required_quantity),
    check (completed_crafts <= coalesce(planned_crafts, completed_crafts)),
    check (
        (kind = 'recipe' and recipe_id is not null and engine_type is not null
            and recipe_ingredients is not null and planned_crafts is not null
            and output_yield is not null)
        or
        (kind in ('storage', 'any') and recipe_id is null and engine_type is null
            and recipe_ingredients is null and planned_crafts is null
            and output_yield is null)
    )
);

create index craft_job_node_job_id_idx on craft_job_node (job_id, key);
create index craft_job_node_schedulable_idx
    on craft_job_node (state, retry_at, created_at)
    where state in ('pending', 'blocked', 'retry_wait');

create table craft_job_operation (
    id uuid primary key,
    node_id uuid not null references craft_job_node(id) on delete cascade,
    work_unit_id uuid,
    kind craft_operation_kind not null,
    attempt integer not null check (attempt > 0),
    state craft_operation_state not null default 'queued',
    completed_amount integer not null default 0 check (completed_amount >= 0),
    payload jsonb not null default '{}'::jsonb,
    checkpoint jsonb not null default '{}'::jsonb,
    error text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    completed_at timestamptz,
    unique (node_id, kind, attempt)
);

create unique index craft_job_operation_active_work_unit_idx
    on craft_job_operation (work_unit_id)
    where work_unit_id is not null and state in ('queued', 'claimed', 'running');

create table craft_processing_lease (
    region_id uuid primary key references container_region(id) on delete cascade,
    node_id uuid not null unique references craft_job_node(id) on delete cascade,
    leased_at timestamptz not null default now(),
    indexed_after_startup boolean not null default false
);
