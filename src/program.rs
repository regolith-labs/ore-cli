use std::convert::TryFrom;

use ore_api::consts::{EPOCH_DURATION, BUS_ADDRESSES};
use owo_colors::OwoColorize;
use tabled::{Table, settings::{Style, Color, Border, object::{Rows, Columns}, Highlight, style::{BorderColor, LineText}, Alignment, Remove}};

use crate::{utils::{get_config, amount_u64_to_f64, format_timestamp, get_bus, TableData}, Miner};


impl Miner {
    pub async fn program(&self) {
        // Aggregate data
        let mut data = vec![];
        self.fetch_config_data(&mut data).await;
        let len1 = data.len();
        self.fetch_busses_data(&mut data).await;
        let len2 = data.len();
        self.fetch_rewards_data(&mut data).await;

        // Build table
        let mut table = Table::new(data);
        table.with(Remove::row(Rows::first()));
        table.modify(Columns::single(1), Alignment::right());
        table.with(Style::blank());
        let title_color = Color::try_from(" ".bold().black().on_white().to_string()).unwrap();
        
        // Config title
        table.with(Highlight::new(Rows::first()).color(BorderColor::default().top(Color::FG_WHITE)));
        table.with(Highlight::new(Rows::first()).border(Border::new().top('━')));
        table.with(LineText::new("Config", Rows::first()).color(title_color.clone()));

        // Busses title
        table.with(Highlight::new(Rows::single(len1)).color(BorderColor::default().top(Color::FG_WHITE)));
        table.with(Highlight::new(Rows::single(len1)).border(Border::new().top('━')));
        table.with(LineText::new("Busses", Rows::single(len1)).color(title_color.clone()));

        // Reward rate title
        table.with(Highlight::new(Rows::single(len2)).color(BorderColor::default().top(Color::FG_WHITE)));
        table.with(Highlight::new(Rows::single(len2)).border(Border::new().top('━')));
        table.with(LineText::new("Reward rates", Rows::single(len2)).color(title_color));

        println!("{table}\n");
    }

    async fn fetch_config_data(&self, data: &mut Vec<TableData>) {
        let config = get_config(&self.rpc_client).await;
        data.push(TableData {
            key: "Last reset at".to_string(),
            value: format_timestamp(config.last_reset_at),
        });
        data.push(TableData {
            key: "Min difficulty".to_string(),
            value: config.min_difficulty.to_string(),
        });
        data.push(TableData {
            key: "Epoch duration".to_string(),
            value: format!("{} sec", EPOCH_DURATION),
        });
    }

    async fn fetch_busses_data(&self, data: &mut Vec<TableData>) {
        for address in BUS_ADDRESSES.iter() {
            let bus = get_bus(&self.rpc_client, *address).await;
            let rewards = amount_u64_to_f64(bus.rewards);
            data.push(TableData {
                key: format!("{}", bus.id),
                value: format!("{:.11} ORE", rewards),
            });
        }
    }

    async fn fetch_rewards_data(&self, data: &mut Vec<TableData>) {
        let config = get_config(&self.rpc_client).await;
        for i in 0..32 {
            let reward_rate = config.base_reward_rate.saturating_mul(2u64.saturating_pow(i));
            let amount = amount_u64_to_f64(reward_rate).min(1.0);
            data.push(TableData {
                key: format!("{}{}", config.min_difficulty as u32 + i, if amount >= 1.0 { "+" } else { "" }),
                value: format!("{:.11} ORE", amount),
            });
            if amount >= 1.0 {
                break;
            }
        }
    }
}
