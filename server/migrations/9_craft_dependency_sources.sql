alter table craft_job_operation
    add column source_node_id uuid references craft_job_node(id) on delete set null;

create index craft_job_operation_source_node_id_idx
    on craft_job_operation (source_node_id)
    where source_node_id is not null;
