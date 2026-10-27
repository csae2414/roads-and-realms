use serde::{Deserialize, Serialize};

macro_rules! all_enum {
    ($(#[$m:meta])* pub enum $name:ident { $($v:ident),* $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[repr(u8)]
        pub enum $name { $($v),* }
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$v),*];
            pub const COUNT: usize = Self::ALL.len();
            pub fn index(self) -> usize { self as usize }
            pub fn name(self) -> &'static str {
                match self { $($name::$v => stringify!($v)),* }
            }
        }
    };
}

all_enum! {
    /// Wares, in the original's inventory order (the four nation shields are one ware here;
    /// the player's nation picks the look).
    pub enum Ware {
        Beer, Tongs, Hammer, Axe, Saw, PickAxe, Shovel, Crucible, RodAndLine, Scythe,
        Water, Cleaver, Rollingpin, Bow, Boat, Sword, Iron, Flour, Fish, Bread,
        Shield, Wood, Boards, Stones, Grain, Coins, Gold, IronOre, Coal, Meat, Ham,
    }
}

impl Ware {
    pub fn is_tool(self) -> bool {
        use Ware::*;
        matches!(
            self,
            Tongs | Hammer | Axe | Saw | PickAxe | Shovel | Crucible | RodAndLine | Scythe | Cleaver | Rollingpin | Bow
        )
    }

    pub fn is_food(self) -> bool {
        matches!(self, Ware::Fish | Ware::Meat | Ware::Bread)
    }
}

all_enum! {
    pub enum Job {
        Helper, Woodcutter, Fisher, Forester, Carpenter, Stonemason, Hunter, Farmer, Miller,
        Baker, Butcher, Miner, Brewer, PigBreeder, DonkeyBreeder, IronFounder, Minter,
        Metalworker, Armorer, Builder, Planer, Private, PrivateFirstClass, Sergeant, Officer,
        General, Geologist, Shipwright, Scout, PackDonkey,
    }
}

impl Job {
    pub const SOLDIERS: [Job; 5] = [Job::Private, Job::PrivateFirstClass, Job::Sergeant, Job::Officer, Job::General];

    pub fn soldier_rank(self) -> Option<u8> {
        Self::SOLDIERS.iter().position(|j| *j == self).map(|r| r as u8)
    }
}

all_enum! {
    pub enum BuildingType {
        Headquarters, Barracks, Guardhouse, Watchtower, Fortress, LookoutTower, Catapult,
        GraniteMine, CoalMine, IronMine, GoldMine, Woodcutter, Fishery, Quarry, Forester,
        Slaughterhouse, Hunter, Brewery, Armory, Metalworks, IronSmelter, PigFarm, Storehouse,
        Mill, Bakery, Sawmill, Mint, Well, Shipyard, Farm, DonkeyBreeder, Harbor,
    }
}

impl BuildingType {
    pub fn is_warehouse(self) -> bool {
        matches!(self, BuildingType::Headquarters | BuildingType::Storehouse | BuildingType::Harbor)
    }
}

/// Building footprint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Size {
    Hut,
    House,
    Castle,
    Mine,
    Harbor,
}
