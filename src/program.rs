use std::convert::TryFrom;

use ore_api::consts::{EPOCH_DURATION, BUS_ADDRESSES};
use owo_colors::OwoColorize;
use tabled::{Table, settings::{Style, Color, Border, object::{Rows, Columns}, Highlight, style::{BorderColor, LineText}, themes::ColumnNames, Alignment}, Tabled};

use crate::{utils::{get_config, amount_u64_to_f64, format_timestamp, get_bus}, Miner};

#[derive(Tabled)]
struct Data {
    key: String,
    value: String,
}

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
        table.with(Style::blank());
        table.modify(Columns::single(1), Alignment::right());
        let title_color = Color::try_from(" ".bold().black().on_white().to_string()).unwrap();
        
        // Config title
        table.with(Highlight::new(Rows::first()).color(BorderColor::default().top(Color::FG_WHITE)));
        table.with(Highlight::new(Rows::first()).border(Border::new().top('━')));
        table.with(LineText::new("Config", Rows::first()).color(title_color.clone()));

        // Busses title
        table.with(Highlight::new(Rows::single(len1 + 1)).color(BorderColor::default().top(Color::FG_WHITE)));
        table.with(Highlight::new(Rows::single(len1 + 1)).border(Border::new().top('━')));
        table.with(LineText::new("Bus balances", Rows::single(len1 + 1)).color(title_color.clone()));

        // Reward rate title
        table.with(Highlight::new(Rows::single(len2 + 1)).color(BorderColor::default().top(Color::FG_WHITE)));
        table.with(Highlight::new(Rows::single(len2 + 1)).border(Border::new().top('━')));
        table.with(LineText::new("Reward rates", Rows::single(len2 + 1)).color(title_color));

        println!("{table}\n");
    }

    async fn fetch_config_data(&self, data: &mut Vec<Data>) {
        let config = get_config(&self.rpc_client).await;
        data.push(Data {
            key: "Last reset at".to_string(),
            value: format_timestamp(config.last_reset_at),
        });
        data.push(Data {
            key: "Min difficulty".to_string(),
            value: config.min_difficulty.to_string(),
        });
        data.push(Data {
            key: "Epoch duration".to_string(),
            value: EPOCH_DURATION.to_string(),
        });
    }

    async fn fetch_busses_data(&self, data: &mut Vec<Data>) {
        for address in BUS_ADDRESSES.iter() {
            let bus = get_bus(&self.rpc_client, *address).await;
            let rewards = amount_u64_to_f64(bus.rewards);
            data.push(Data {
                key: format!("{}", bus.id),
                value: format!("{:.11} ORE", rewards),
            });
        }
    }

    async fn fetch_rewards_data(&self, data: &mut Vec<Data>) {
        let config = get_config(&self.rpc_client).await;
        for i in 0..32 {
            let reward_rate = config.base_reward_rate.saturating_mul(2u64.saturating_pow(i));
            let amount = amount_u64_to_f64(reward_rate).min(1.0);
            data.push(Data {
                key: format!("{}{}", config.min_difficulty as u32 + i, if amount >= 1.0 { "+" } else { "" }),
                value: format!("{:.11} ORE", amount),
            });
            if amount >= 1.0 {
                break;
            }
        }
    }
}
