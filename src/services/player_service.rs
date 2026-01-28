use crate::{
    errors::AppError,
    models::tournament::Title,
    repositories::player_repo::{self, DbPlayer, update_fide_player},
    responses::FidePlayer,
};
use chrono::{DateTime, Datelike, TimeDelta, Utc};
use reqwest::Client;
use scraper::{Html, Selector};

fn split_name(full_name: String) -> (String, String) {
    if let Some((last, first)) = full_name.split_once(',') {
        (first.trim().to_string(), last.trim().to_string())
    } else {
        if let Some((l, f)) = full_name.rsplit_once(' ') {
            (f.trim().to_string(), l.trim().to_string())
        } else {
            ("".to_string(), full_name.trim().to_string())
        }
    }
}

pub enum FidePlayerCheck {
    Exists(u32),
    Updated(DbPlayer),
}

pub async fn check_fide_player_exists(
    pool: &sqlx::SqlitePool,
    fide_id: i64,
    client: &Client,
) -> Result<Option<FidePlayerCheck>, AppError> {
    match player_repo::get_player_by_fide_id(pool, fide_id)
        .await
        .map_err(|e| Into::<AppError>::into(e))?
    {
        Some(player) => {
            let now = Utc::now();
            let last_update = DateTime::from_timestamp_secs(player.updated_at as i64).unwrap();
            tracing::info!(
                "now - last_update of {:?} = {:?}",
                player.fide_id,
                now - last_update
            );
            let should_update = if now.year() == last_update.year() {
                now.month() != last_update.month()
            } else {
                true
            };
            if should_update {
                let updated_player = scrape_fide_player(client, fide_id).await?;
                let updated_at = update_fide_player(pool, updated_player.into()).await?;
                Ok(Some(FidePlayerCheck::Updated(DbPlayer {
                    id: player.id,
                    first_name: player.first_name,
                    last_name: player.last_name,
                    updated_at: updated_at as u32,
                    federation: player.federation,
                    fide_id: player.fide_id,
                    title: player.title,
                    rating: player.rating,
                    rating_rapid: player.rating_rapid,
                    rating_blitz: player.rating_blitz,
                })))
            } else {
                Ok(Some(FidePlayerCheck::Exists(player.id as u32)))
            }
        }
        None => Ok(None),
    }
}

pub async fn scrape_fide_player(client: &Client, fide_id: i64) -> Result<FidePlayer, AppError> {
    let url = format!("https://ratings.fide.com/profile/{}", fide_id);

    let res = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::FideScrapeFailed(format!("Request error: {}", e)))?;

    if !res.status().is_success() {
        return Err(AppError::FideScrapeFailed("Player not found".to_string()));
    }

    let html_content = res
        .text()
        .await
        .map_err(|e| AppError::FideScrapeFailed(format!("Request error: {}", e)))?;

    let document = Html::parse_document(&html_content);

    let name_sel = Selector::parse(r"h1.player-title")
        .map_err(|_| AppError::FideScrapeFailed("Invalid css selector".to_string()))?;
    let fed_sel = Selector::parse(r"div.profile-info-country")
        .map_err(|_| AppError::FideScrapeFailed("Invalid css selector".to_string()))?;
    let title_sel = Selector::parse(r"div.profile-info-title > p")
        .map_err(|_| AppError::FideScrapeFailed("Invalid css selector".to_string()))?;
    let standard_sel = Selector::parse(r"div.profile-standart.profile-game > p")
        .map_err(|_| AppError::FideScrapeFailed("Invalid css selector".to_string()))?;
    let rapid_sel = Selector::parse(r"div.profile-rapid.profile-game > p")
        .map_err(|_| AppError::FideScrapeFailed("Invalid css selector".to_string()))?;
    let blitz_sel = Selector::parse(r"div.profile-blitz.profile-game > p")
        .map_err(|_| AppError::FideScrapeFailed("Invalid css selector".to_string()))?;

    let full_name = document
        .select(&name_sel)
        .next()
        .ok_or(AppError::FideScrapeFailed(
            "Missing player name".to_string(),
        ))?
        .text()
        .collect::<String>()
        .trim()
        .to_string();

    let (first_name, last_name) = split_name(full_name);

    let federation = match document
        .select(&fed_sel)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
    {
        Some(f) => full_name_to_fide_code(f.as_str()),
        None => None,
    };

    let title = document
        .select(&title_sel)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty() && s != "-");

    tracing::info!("Title: {:?}", title);

    let title = if let Some(t) = title {
        Title::from_str(t)
    } else {
        Title::Untitled
    };

    // Helper to parse rating
    let parse_rating = |sel: &Selector| -> Option<u32> {
        document
            .select(sel)
            .next()
            .and_then(|el| el.text().collect::<String>().trim().parse::<u32>().ok())
    };

    let rating = parse_rating(&standard_sel);
    let rating_rapid = parse_rating(&rapid_sel);
    let rating_blitz = parse_rating(&blitz_sel);

    Ok(FidePlayer {
        fide_id,
        first_name,
        last_name,
        federation,
        title: match title {
            Title::Untitled => None,
            _ => Some(title.to_string()),
        },
        rating,
        rating_rapid,
        rating_blitz,
    })
}

fn full_name_to_fide_code(full_name: &str) -> Option<String> {
    match full_name.trim() {
        "Afghanistan" => Some("AFG".to_string()),
        "Albania" => Some("ALB".to_string()),
        "Algeria" => Some("ALG".to_string()),
        "Andorra" => Some("AND".to_string()),
        "Angola" => Some("ANG".to_string()),
        "Antigua and Barbuda" => Some("ANT".to_string()),
        "Argentina" => Some("ARG".to_string()),
        "Armenia" => Some("ARM".to_string()),
        "Aruba" => Some("ARU".to_string()),
        "Australia" => Some("AUS".to_string()),
        "Austria" => Some("AUT".to_string()),
        "Azerbaijan" => Some("AZE".to_string()),
        "Bahamas" => Some("BAH".to_string()),
        "Bahrain" => Some("BRN".to_string()),
        "Bangladesh" => Some("BAN".to_string()),
        "Barbados" => Some("BAR".to_string()),
        "Belarus" => Some("BLR".to_string()),
        "Belgium" => Some("BEL".to_string()),
        "Belize" => Some("BIZ".to_string()),
        "Bermuda" => Some("BER".to_string()),
        "Bhutan" => Some("BHU".to_string()),
        "Bolivia" => Some("BOL".to_string()),
        "Bosnia and Herzegovina" => Some("BIH".to_string()),
        "Botswana" => Some("BOT".to_string()),
        "Brazil" => Some("BRA".to_string()),
        "British Virgin Islands" => Some("IVB".to_string()),
        "Brunei Darussalam" => Some("BRU".to_string()),
        "Bulgaria" => Some("BUL".to_string()),
        "Burundi" => Some("BDI".to_string()),
        "Cambodia" => Some("CAM".to_string()),
        "Cameroon" => Some("CMR".to_string()),
        "Canada" => Some("CAN".to_string()),
        "Cape Verde" => Some("CPV".to_string()),
        "Cayman Islands" => Some("CAY".to_string()),
        "Central African Republic" => Some("CAF".to_string()),
        "Chad" => Some("CHA".to_string()),
        "Chile" => Some("CHI".to_string()),
        "China" => Some("CHN".to_string()),
        "Chinese Taipei" => Some("TPE".to_string()),
        "Colombia" => Some("COL".to_string()),
        "Comoros Islands" => Some("COM".to_string()),
        "Costa Rica" => Some("CRC".to_string()),
        "Cote d’Ivoire" => Some("CIV".to_string()),
        "Croatia" => Some("CRO".to_string()),
        "Cuba" => Some("CUB".to_string()),
        "Cyprus" => Some("CYP".to_string()),
        "Czech Republic" => Some("CZE".to_string()),
        "Democratic Republic of the Congo" => Some("COD".to_string()),
        "Denmark" => Some("DEN".to_string()),
        "Djibouti" => Some("DJI".to_string()),
        "Dominica" => Some("DMA".to_string()),
        "Dominican Republic" => Some("DOM".to_string()),
        "Ecuador" => Some("ECU".to_string()),
        "Egypt" => Some("EGY".to_string()),
        "El Salvador" => Some("ESA".to_string()),
        "England" => Some("ENG".to_string()),
        "Equatorial Guinea" => Some("GEQ".to_string()),
        "Eritrea" => Some("ERI".to_string()),
        "Estonia" => Some("EST".to_string()),
        "Eswatini" => Some("SWZ".to_string()),
        "Ethiopia" => Some("ETH".to_string()),
        "Faroe Islands" => Some("FAI".to_string()),
        "Fiji" => Some("FIJ".to_string()),
        "Finland" => Some("FIN".to_string()),
        "France" => Some("FRA".to_string()),
        "Gabon" => Some("GAB".to_string()),
        "Gambia" => Some("GAM".to_string()),
        "Georgia" => Some("GEO".to_string()),
        "Germany" => Some("GER".to_string()),
        "Ghana" => Some("GHA".to_string()),
        "Greece" => Some("GRE".to_string()),
        "Grenada" => Some("GRN".to_string()),
        "Guam" => Some("GUM".to_string()),
        "Guatemala" => Some("GUA".to_string()),
        "Guernsey" => Some("GCI".to_string()),
        "Guyana" => Some("GUY".to_string()),
        "Haiti" => Some("HAI".to_string()),
        "Honduras" => Some("HON".to_string()),
        "Hong Kong, China" => Some("HKG".to_string()),
        "Hungary" => Some("HUN".to_string()),
        "Iceland" => Some("ISL".to_string()),
        "India" => Some("IND".to_string()),
        "Indonesia" => Some("INA".to_string()),
        "Iran" => Some("IRI".to_string()),
        "Iraq" => Some("IRQ".to_string()),
        "Ireland" => Some("IRL".to_string()),
        "Israel" => Some("ISR".to_string()),
        "Italy" => Some("ITA".to_string()),
        "Jamaica" => Some("JAM".to_string()),
        "Japan" => Some("JPN".to_string()),
        "Jersey" => Some("JCI".to_string()),
        "Jordan" => Some("JOR".to_string()),
        "Kazakhstan" => Some("KAZ".to_string()),
        "Kenya" => Some("KEN".to_string()),
        "Kosovo" => Some("KOS".to_string()),
        "Kuwait" => Some("KUW".to_string()),
        "Kyrgyzstan" => Some("KGZ".to_string()),
        "Laos" => Some("LAO".to_string()),
        "Latvia" => Some("LAT".to_string()),
        "Lebanon" => Some("LBN".to_string()),
        "Lesotho" => Some("LES".to_string()),
        "Liberia" => Some("LBR".to_string()),
        "Libya" => Some("LBA".to_string()),
        "Liechtenstein" => Some("LIE".to_string()),
        "Lithuania" => Some("LTU".to_string()),
        "Luxembourg" => Some("LUX".to_string()),
        "Macau, China" => Some("MAC".to_string()),
        "Madagascar" => Some("MAD".to_string()),
        "Malawi" => Some("MAW".to_string()),
        "Malaysia" => Some("MAS".to_string()),
        "Maldives" => Some("MDV".to_string()),
        "Mali" => Some("MLI".to_string()),
        "Malta" => Some("MLT".to_string()),
        "Mauritania" => Some("MTN".to_string()),
        "Mauritius" => Some("MRI".to_string()),
        "Mexico" => Some("MEX".to_string()),
        "Moldova" => Some("MDA".to_string()),
        "Monaco" => Some("MNC".to_string()),
        "Mongolia" => Some("MGL".to_string()),
        "Montenegro" => Some("MNE".to_string()),
        "Morocco" => Some("MAR".to_string()),
        "Mozambique" => Some("MOZ".to_string()),
        "Myanmar" => Some("MYA".to_string()),
        "Namibia" => Some("NAM".to_string()),
        "Nauru" => Some("NRU".to_string()),
        "Nepal" => Some("NEP".to_string()),
        "Netherlands" => Some("NED".to_string()),
        "Netherlands Antilles" => Some("AHO".to_string()),
        "New Zealand" => Some("NZL".to_string()),
        "Nicaragua" => Some("NCA".to_string()),
        "Niger" => Some("NIG".to_string()),
        "Nigeria" => Some("NGR".to_string()),
        "North Macedonia" => Some("MKD".to_string()),
        "Norway" => Some("NOR".to_string()),
        "Oman" => Some("OMA".to_string()),
        "Pakistan" => Some("PAK".to_string()),
        "Palau" => Some("PLW".to_string()),
        "Palestine" => Some("PLE".to_string()),
        "Panama" => Some("PAN".to_string()),
        "Papua New Guinea" => Some("PNG".to_string()),
        "Paraguay" => Some("PAR".to_string()),
        "Peru" => Some("PER".to_string()),
        "Philippines" => Some("PHI".to_string()),
        "Poland" => Some("POL".to_string()),
        "Portugal" => Some("POR".to_string()),
        "Puerto Rico" => Some("PUR".to_string()),
        "Qatar" => Some("QAT".to_string()),
        "Romania" => Some("ROU".to_string()),
        "Russia" => Some("RUS".to_string()),
        "Rwanda" => Some("RWA".to_string()),
        "Saint Kitts and Nevis" => Some("SKN".to_string()),
        "Saint Lucia" => Some("LCA".to_string()),
        "Saint Vincent and the Grenadines" => Some("VIN".to_string()),
        "San Marino" => Some("SMR".to_string()),
        "Sao Tome and Principe" => Some("STP".to_string()),
        "Saudi Arabia" => Some("KSA".to_string()),
        "Scotland" => Some("SCO".to_string()),
        "Senegal" => Some("SEN".to_string()),
        "Serbia" => Some("SRB".to_string()),
        "Seychelles" => Some("SEY".to_string()),
        "Sierra Leone" => Some("SLE".to_string()),
        "Singapore" => Some("SGP".to_string()),
        "Slovakia" => Some("SVK".to_string()),
        "Slovenia" => Some("SLO".to_string()),
        "Solomon Islands" => Some("SOL".to_string()),
        "Somalia" => Some("SOM".to_string()),
        "South Africa" => Some("RSA".to_string()),
        "South Korea" => Some("KOR".to_string()),
        "South Sudan" => Some("SSD".to_string()),
        "Spain" => Some("ESP".to_string()),
        "Sri Lanka" => Some("SRI".to_string()),
        "Sudan" => Some("SUD".to_string()),
        "Suriname" => Some("SUR".to_string()),
        "Sweden" => Some("SWE".to_string()),
        "Switzerland" => Some("SUI".to_string()),
        "Syria" => Some("SYR".to_string()),
        "Tajikistan" => Some("TJK".to_string()),
        "Tanzania" => Some("TAN".to_string()),
        "Thailand" => Some("THA".to_string()),
        "Timor-Leste" => Some("TLS".to_string()),
        "Togo" => Some("TOG".to_string()),
        "Tonga" => Some("TGA".to_string()),
        "Trinidad and Tobago" => Some("TTO".to_string()),
        "Tunisia" => Some("TUN".to_string()),
        "Turkiye" => Some("TUR".to_string()),
        "Turkmenistan" => Some("TKM".to_string()),
        "Uganda" => Some("UGA".to_string()),
        "Ukraine" => Some("UKR".to_string()),
        "United Arab Emirates" => Some("UAE".to_string()),
        "United States of America" => Some("USA".to_string()),
        "Uruguay" => Some("URU".to_string()),
        "US Virgin Islands" => Some("ISV".to_string()),
        "Uzbekistan" => Some("UZB".to_string()),
        "Vanuatu" => Some("VAN".to_string()),
        "Venezuela" => Some("VEN".to_string()),
        "Vietnam" => Some("VIE".to_string()),
        "Wales" => Some("WLS".to_string()),
        "Yemen" => Some("YEM".to_string()),
        "Zambia" => Some("ZAM".to_string()),
        "Zimbabwe" => Some("ZIM".to_string()),
        "Burkina Faso" => Some("BUR".to_string()),
        // Special: FIDE flag (for players without a national federation or under FIDE directly)
        "FIDE" => Some("FID".to_string()),
        _ => None,
    }
}
