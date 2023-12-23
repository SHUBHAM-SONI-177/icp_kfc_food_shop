#[macro_use]
extern crate serde;

use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::collections::BTreeMap;
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct FoodItem {
    id: u64,
    name: String,
    description: String,
    price: f64,
    available: bool,
    created_at: u64,  // Fixed typo in field name
}

impl Storable for FoodItem {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for FoodItem {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static FOOD_MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static FOOD_ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(FOOD_MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter for food items")
    );

    static FOOD_MENU: RefCell<BTreeMap<u64, FoodItem>> = RefCell::new(BTreeMap::new());

}

fn do_insert_food_item(item: &FoodItem) {
    FOOD_MENU.with(|service| service.borrow_mut().insert(item.id, item.clone()));
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct FoodItemPayload {
    name: String,
    description: String,
    price: f64,
}

#[ic_cdk::query]
fn get_food_item(id: u64) -> Result<FoodItem, Error> {
    match _get_food_item(&id) {
        Some(item) => Ok(item),
        None => Err(Error::NotFound {
            msg: format!("a food item with id={} not found", id),
        }),
    }
}

fn _get_food_item(id: &u64) -> Option<FoodItem> {
    FOOD_MENU.with(|s| s.borrow().get(id).cloned())
}

#[ic_cdk::update]
fn add_food_item(item: FoodItemPayload) -> Option<FoodItem> {
    let id = FOOD_ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("cannot increment id counter for food items");
    let food_item = FoodItem {
        id,
        name: item.name,
        description: item.description,
        price: item.price,
        available: true,
        created_at: time(),
    };
    do_insert_food_item(&food_item);
    Some(food_item)
}

#[ic_cdk::update]
fn update_food_item(id: u64, payload: FoodItemPayload) -> Result<FoodItem, Error> {
    match FOOD_MENU.with(|service| service.borrow().get(&id).cloned()) {
        Some(mut food_item) => {
            food_item.name = payload.name;
            food_item.description = payload.description;
            food_item.price = payload.price;
            do_insert_food_item(&food_item);
            Ok(food_item)
        }
        None => Err(Error::NotFound {
            msg: format!("couldn't update a food item with id={}. item not found", id),
        }),
    }
}

#[ic_cdk::update]
fn delete_food_item(id: u64) -> Result<FoodItem, Error> {
    match FOOD_MENU.with(|service| service.borrow_mut().remove(&id)) {
        Some(food_item) => Ok(food_item),
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't delete a food item with id={}. item not found.",
                id
            ),
        }),
    }
}

#[ic_cdk::query]
fn get_menu() -> Vec<FoodItem> {
    FOOD_MENU.with(|service| {
        service
            .borrow()
            .iter()
            .map(|(_, value)| value.clone())
            .collect()
    })
}

#[ic_cdk::query]
fn search_food_item_by_name(name: String) -> Vec<FoodItem> {
    FOOD_MENU.with(|service| {
        let map = service.borrow();
        map.iter()
            .filter_map(|(_, item)| {
                if item.name.contains(&name) {
                    Some(item.clone())
                } else {
                    None
                }
            })
            .collect()
    })
}

#[ic_cdk::query]
fn search_food_item_by_below_price(price: f64) -> Vec<FoodItem> {
    FOOD_MENU.with(|service| {
        let map = service.borrow();
        map.iter()
            .filter_map(|(_, item)| {
                if item.price <= price {
                    Some(item.clone())
                } else {
                    None
                }
            })
            .collect()
    })
}

#[ic_cdk::query]
fn search_food_item_by_above_price(price: f64) -> Vec<FoodItem> {
    FOOD_MENU.with(|service| {
        let map = service.borrow();
        map.iter()
            .filter_map(|(_, item)| {
                if item.price >= price {
                    Some(item.clone())
                } else {
                    None
                }
            })
            .collect()
    })
}

#[ic_cdk::update]
fn order_food_item(id: u64) -> Result<(), Error> {
    match FOOD_MENU.with(|service| {
        service
            .borrow_mut()
            .get_mut(&id)
            .filter(|food_item| food_item.available)
            .map(|food_item| {
                // Add logic for processing the order, updating availability, etc.
                food_item.available = false;
            })
    }) {
        Some(()) => Ok(()),
        None => Err(Error::NotFound {
            msg: format!("couldn't order a food item with id={}. item not found or not available for order", id),
        }),
    }
}

#[ic_cdk::update]
fn receive_food_item(id: u64) -> Result<(), Error> {
    match FOOD_MENU.with(|service| {
        service
            .borrow_mut()
            .get_mut(&id)
            .filter(|food_item| !food_item.available)
            .map(|food_item| {
                // Add logic for processing the received item, updating availability, etc.
                food_item.available = true;
            })
    }) {
        Some(()) => Ok(()),
        None => Err(Error::NotFound {
            msg: format!("couldn't receive a food item with id={}. item not found or already marked as available", id),
        }),
    }
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
    ItemNotAvailable { msg: String },  // Added variant for item not available
}

ic_cdk::export_candid!();
