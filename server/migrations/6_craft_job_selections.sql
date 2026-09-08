alter table craft_job
    add column if not exists selections jsonb not null default '{}'::jsonb;