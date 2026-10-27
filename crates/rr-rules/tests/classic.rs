use rr_rules::{BuildingType as B, Job, Rules, Size, Ware};

#[test]
fn classic_rules_load_and_validate() {
    let r = Rules::classic();
    assert_eq!(r.building(B::Woodcutter).size, Size::Hut);
    assert_eq!(r.building(B::Woodcutter).cost(), (2, 0));
    assert_eq!(r.building(B::Fortress).cost(), (4, 7));
    assert_eq!(r.building(B::Fortress).size, Size::Castle);
    assert_eq!(r.building(B::Sawmill).size, Size::House);
    assert_eq!(r.building(B::GoldMine).size, Size::Mine);
    assert_eq!(r.building(B::Farm).size, Size::Castle);
    assert_eq!(r.building(B::Sawmill).worker, Some(Job::Carpenter));
    assert_eq!(r.building(B::Sawmill).inputs, vec![Ware::Wood]);
    assert_eq!(r.building(B::Sawmill).output, Some(Ware::Boards));
}

#[test]
fn mines_eat_any_food() {
    let r = Rules::classic();
    let m = r.building(B::CoalMine);
    assert_eq!(m.inputs, vec![Ware::Fish, Ware::Meat, Ware::Bread]);
    assert!(!m.use_one_each);
    assert_eq!(m.slots_per_input, 2);
    assert_eq!(m.output, Some(Ware::Coal));
}

#[test]
fn military_values() {
    let r = Rules::classic();
    let m = r.building(B::Guardhouse).military.as_ref().unwrap();
    assert_eq!((m.soldiers, m.coins, m.radius), (3, 2, 9));
    let hq = r.building(B::Headquarters).military.as_ref().unwrap();
    assert_eq!(hq.radius, 9);
    assert_eq!(r.military.hp, [3, 4, 5, 6, 7]);
}

#[test]
fn job_timings_from_rttr() {
    let r = Rules::classic();
    let j = r.job(Job::Woodcutter);
    assert_eq!((j.tool, j.work, j.wait1, j.wait2), (Some(Ware::Axe), 148, 789, 5));
    assert_eq!(r.job(Job::Helper).tool, None);
    assert_eq!(r.job(Job::Builder).tool, Some(Ware::Hammer));
}

#[test]
fn hq_start_goods_normal() {
    let r = Rules::classic();
    let s = &r.start;
    assert_eq!(s.ware(Ware::Boards), 44);
    assert_eq!(s.ware(Ware::Stones), 68);
    assert_eq!(s.job(Job::Helper), 52);
    assert_eq!(s.job(Job::Private), 46);
    assert_eq!(s.job(Job::Builder), 10);
}

#[test]
fn every_enum_value_has_a_definition() {
    let r = Rules::classic();
    for b in B::ALL {
        let _ = r.building(*b);
    }
    for j in Job::ALL {
        let _ = r.job(*j);
    }
    assert_eq!(Ware::ALL.len(), 31);
}

#[test]
fn rules_hash_is_stable_and_detects_changes() {
    let a = Rules::classic();
    let b = Rules::classic();
    assert_eq!(a.hash(), b.hash());
    let mut c = Rules::classic();
    c.military.hp[0] = 9;
    assert_ne!(a.hash(), c.hash());
}

#[test]
fn floats_are_rejected() {
    let text = rr_rules::CLASSIC_TOML.replace("work = 148", "work = 148.5");
    assert!(Rules::from_toml(&text).is_err());
}
