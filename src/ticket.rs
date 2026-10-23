//! The ticket the POS sends: what the employee actually rang in.

use anyhow::bail;
use serde::{Deserialize, Serialize};

use crate::menu::{Menu, Size};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TicketComponent {
    pub item: String,
    #[serde(default)]
    pub size: Option<Size>,
    #[serde(default)]
    pub modifiers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TicketMeal {
    pub side: TicketComponent,
    #[serde(default)]
    pub drink: Option<TicketComponent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TicketLine {
    pub item: String,
    pub qty: u32,
    #[serde(default)]
    pub size: Option<Size>,
    #[serde(default)]
    pub modifiers: Vec<String>,
    #[serde(default)]
    pub meal: Option<TicketMeal>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Ticket {
    pub lines: Vec<TicketLine>,
}

impl Ticket {
    /// Rejects anything the menu doesn't know, so a bad POS payload can't
    /// produce a confident-looking check.
    pub fn validate(&self, menu: &Menu) -> anyhow::Result<()> {
        for line in &self.lines {
            if line.qty == 0 {
                bail!("ticket line {} has quantity 0", line.item);
            }
            check_component(menu, &line.item, &line.modifiers)?;
            if let Some(meal) = &line.meal {
                check_component(menu, &meal.side.item, &meal.side.modifiers)?;
                if let Some(d) = &meal.drink {
                    check_component(menu, &d.item, &d.modifiers)?;
                }
            }
        }
        Ok(())
    }
}

fn check_component(menu: &Menu, item: &str, modifiers: &[String]) -> anyhow::Result<()> {
    let Some(it) = menu.item(item) else {
        bail!("unknown item {item}")
    };
    for m in modifiers {
        if !it.modifiers.contains(m) {
            bail!("modifier {m} is not allowed on {item}");
        }
    }
    Ok(())
}
