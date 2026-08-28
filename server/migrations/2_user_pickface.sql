-- A user pickface is a pre-defined list of containers and slots, and what to stock them with.
create table user_pickface(
    container_id uuid primary key,
    constraint fk_container_id foreign key (container_id) references container(id),

    -- ordered list of what item_kind to stock each slot with. empty/air indicates don't stock
    item_slots text[] not null default '{}'
);