alter table craft_job drop constraint craft_job_state_check;

alter table craft_job
    add constraint craft_job_state_check
    check (state in ('queued', 'waiting', 'running', 'completed', 'failed'));