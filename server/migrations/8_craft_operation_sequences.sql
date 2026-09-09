alter table craft_job_operation
    add column sequence integer not null default 0 check (sequence >= 0);

alter table craft_job_operation
    drop constraint craft_job_operation_node_id_kind_attempt_key;

alter table craft_job_operation
    add constraint craft_job_operation_node_kind_attempt_sequence_key
    unique (node_id, kind, attempt, sequence);
