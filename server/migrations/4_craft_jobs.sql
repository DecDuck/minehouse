create table craft_job (
    id uuid primary key,
    target_item_kind text not null,
    target_quantity integer not null check (target_quantity > 0),
    state text not null check (state in ('queued', 'running', 'completed', 'failed')),
    completed_quantity integer not null default 0 check (completed_quantity >= 0),
    error text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index craft_job_created_at_idx on craft_job (created_at desc);