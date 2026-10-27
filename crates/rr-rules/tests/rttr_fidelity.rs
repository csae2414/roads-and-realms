//! Fidelity eval: our classic rules must match the Return to the Roots source for every value
//! we took from it. Skipped when the reference checkout (`../reference/s25client`) is missing.

use rr_rules::{BuildingType, Job, Rules, Size, Ware};
use std::path::PathBuf;

fn src(rel: &str) -> Option<String> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../reference/s25client/libs/s25main").join(rel);
    std::fs::read_to_string(p).ok()
}

fn rttr_job(j: Job) -> &'static str {
    match j {
        Job::Woodcutter => "Woodchopper",
        Job::Forester => "Tree planter",
        Job::Hunter => "Huntsman",
        Job::PigBreeder => "Pig breeder",
        Job::DonkeyBreeder => "Donkey breeder",
        Job::IronFounder => "Iron founder",
        Job::PrivateFirstClass => "Private first class",
        Job::PackDonkey => "Pack donkey",
        other => other.name(),
    }
}

fn rttr_ware(name: &str) -> Option<Ware> {
    Ware::ALL.iter().copied().find(|w| w.name() == name)
}

#[test]
fn job_tools_and_times_match_rttr() {
    let Some(text) = src("gameData/JobConsts.cpp") else {
        eprintln!("skipped: no RTTR checkout");
        return;
    };
    let rules = Rules::classic();
    let mut checked = 0;
    let pieces: Vec<&str> = text.split("gettext_noop(\"").collect();
    for i in 1..pieces.len() {
        let name = pieces[i].split('"').next().unwrap_or("");
        let Some(job) = Job::ALL.iter().copied().find(|j| rttr_job(*j) == name) else { continue };
        // the entry's data is the last `{{...}` before its name
        let prev = pieces[i - 1];
        let Some(start) = prev.rfind("{{") else { continue };
        let data = prev[start + 2..].split('}').next().unwrap_or("");
        if data.trim().is_empty() {
            continue; // soldiers, donkeys: no job data
        }
        let parts: Vec<&str> = data.split(',').map(str::trim).collect();
        let tool = parts[0].strip_prefix("GoodType::").and_then(rttr_ware);
        let nums: Vec<u32> = parts[1..].iter().filter_map(|p| p.parse().ok()).collect();
        let ours = rules.job(job);
        assert_eq!(ours.tool, tool, "{job:?} tool");
        assert_eq!((ours.work, ours.wait1, ours.wait2), (nums[0], nums[1], nums[2]), "{job:?} timings");
        checked += 1;
    }
    assert!(checked >= 24, "only {checked} jobs compared");
}

#[test]
fn building_costs_and_sizes_match_rttr() {
    let Some(consts) = src("gameData/BuildingConsts.h") else {
        eprintln!("skipped: no RTTR checkout");
        return;
    };
    let Some(types) = src("gameTypes/BuildingType.h") else { return };
    let order: Vec<String> = types
        .split("enum class BuildingType")
        .nth(1)
        .unwrap()
        .split("};")
        .next()
        .unwrap()
        .lines()
        .filter_map(|l| {
            let w = l.trim().split([',', ' ', '/']).next()?;
            (!w.is_empty() && w.chars().next()?.is_ascii_uppercase()).then(|| w.to_string())
        })
        .collect();
    let block = |name: &str| consts.split(name).nth(1).unwrap().split(";").next().unwrap().to_string();
    let costs: Vec<(u8, u8)> = {
        let b = block("BUILDING_COSTS");
        let nums: Vec<u8> = b.split(|c: char| !c.is_ascii_digit()).filter_map(|s| s.parse().ok()).collect();
        nums.chunks(2).map(|c| (c[0], c[1])).collect()
    };
    let sizes: Vec<String> = block("BUILDING_SIZE")
        .split("BuildingQuality::")
        .skip(1)
        .map(|s| s.split([',', ' ', '}']).next().unwrap().to_string())
        .collect();
    let rules = Rules::classic();
    let ours_name = |b: BuildingType| match b {
        BuildingType::IronSmelter => "Ironsmelter".to_string(),
        BuildingType::Harbor => "HarborBuilding".to_string(),
        other => other.name().to_string(),
    };
    for b in BuildingType::ALL {
        let i = order.iter().position(|n| *n == ours_name(*b)).unwrap_or_else(|| panic!("{b:?} not in RTTR enum"));
        let def = rules.building(*b);
        if *b != BuildingType::Headquarters {
            assert_eq!(def.cost(), costs[i], "{b:?} cost");
        }
        let size = match sizes[i].as_str() {
            "Hut" => Size::Hut,
            "House" => Size::House,
            "Castle" => Size::Castle,
            "Mine" => Size::Mine,
            "Harbor" => Size::Harbor,
            other => panic!("{other}"),
        };
        assert_eq!(def.size, size, "{b:?} size");
    }
}
