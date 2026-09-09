create table recipe (
    id uuid primary key,
    output_item_kind text not null,
    output_yield integer not null check (output_yield > 0),
    engine_type crafting_engine_type not null default 'crafting_table'
);

create index recipe_output_item_kind_idx on recipe (output_item_kind);

create table recipe_ingredient (
    recipe_id uuid not null references recipe(id) on delete cascade,
    item_kind text not null,
    quantity integer not null check (quantity > 0),
    position integer not null check (position >= 0),
    primary key (recipe_id, position)
);

insert into recipe (id, output_item_kind, output_yield) values
    ('00000000-0000-0000-0000-000000000001', 'minecraft:hopper', 1),
    ('00000000-0000-0000-0000-000000000002', 'minecraft:chest', 1),
    ('00000000-0000-0000-0000-000000000003', 'minecraft:crafting_table', 1),
    ('00000000-0000-0000-0000-000000000004', 'minecraft:piston', 1),
    ('00000000-0000-0000-0000-000000000005', 'minecraft:iron_pickaxe', 1),
    ('00000000-0000-0000-0000-000000000006', 'minecraft:oak_planks', 4),
    ('00000000-0000-0000-0000-000000000007', 'minecraft:oak_planks', 4),
    ('00000000-0000-0000-0000-000000000008', 'minecraft:stick', 4),
    ('00000000-0000-0000-0000-000000000009', 'minecraft:iron_ingot', 1),
    ('00000000-0000-0000-0000-00000000000a', 'minecraft:iron_ingot', 9);

insert into recipe_ingredient (recipe_id, item_kind, quantity, position) values
    ('00000000-0000-0000-0000-000000000001', 'minecraft:iron_ingot', 5, 0),
    ('00000000-0000-0000-0000-000000000001', 'minecraft:chest', 1, 1),
    ('00000000-0000-0000-0000-000000000002', 'minecraft:oak_planks', 8, 0),
    ('00000000-0000-0000-0000-000000000003', 'minecraft:oak_planks', 4, 0),
    ('00000000-0000-0000-0000-000000000004', 'minecraft:oak_planks', 3, 0),
    ('00000000-0000-0000-0000-000000000004', 'minecraft:cobblestone', 4, 1),
    ('00000000-0000-0000-0000-000000000004', 'minecraft:iron_ingot', 1, 2),
    ('00000000-0000-0000-0000-000000000004', 'minecraft:redstone', 1, 3),
    ('00000000-0000-0000-0000-000000000005', 'minecraft:iron_ingot', 3, 0),
    ('00000000-0000-0000-0000-000000000005', 'minecraft:stick', 2, 1),
    ('00000000-0000-0000-0000-000000000006', 'minecraft:oak_log', 1, 0),
    ('00000000-0000-0000-0000-000000000007', 'minecraft:oak_wood', 1, 0),
    ('00000000-0000-0000-0000-000000000008', 'minecraft:oak_planks', 2, 0),
    ('00000000-0000-0000-0000-000000000009', 'minecraft:iron_ore', 1, 0),
    ('00000000-0000-0000-0000-00000000000a', 'minecraft:iron_block', 1, 0);