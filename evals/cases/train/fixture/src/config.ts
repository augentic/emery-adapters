import { ConfigCommon } from "realtime-common";
import { AzureAccessTokenRetrieverConfig } from "auth-common";

export class Config extends ConfigCommon {
    // default: Central(0), South(40), East(19)
    public static filterStations = (process.env.STATIONS || "0,19,40").split(",");

    public static staticCCUrl = process.env.GTFS_CC_STATIC_URL || "https://static-api.transit.example.com";
    public static blockMgtUrl = process.env.BLOCK_MANAGEMENT_URL || "https://block-mgt-api.transit.example.com";

    public static dateFormat = "DD/MM/YYYY";
    public static maxMessageDelay = Number(process.env.MAX_MESSAGE_DELAY_IN_SECONDS || "60");
    public static minMessageDelay = Number(process.env.MIN_MESSAGE_DELAY_IN_SECONDS || "-30");

    public static useConfluentKafkaConfig = process.env.IS_LOCAL !== "true";

    public static rootElement = "CCO";
    public static xmlOptions = {
        arrayAccessFormPaths: [`${Config.rootElement}.ActualizarDatosTren.pasoTren`],
    };

    public static UNMAPPED_STATION_ID = "unmapped";
    public static DEPARTURE_LOCATION_OVERWRITE: { [key: number]: { stop_lat: number, stop_lon: number } } = {
        133: {
            stop_lat: -36.84448,
            stop_lon: 174.76915,
        },
        134: {
            stop_lat: -37.20299,
            stop_lon: 174.90990,
        },
        9218: {
            stop_lat: -36.99412,
            stop_lon: 174.8770,
        },
    };

    public static STATION_ID_TO_STOP_CODE_MAP: { [key: number]: string } = {
        0: "133",
        2: "115",
        3: "102",
        4: "605",
        5: Config.UNMAPPED_STATION_ID, // North not mapped and does not have long/lat in old mapping
        6: "244",
        7: "122",
        8: "104",
        9: "105",
        10: "129",
        11: "125",
        12: "128",
        13: "127",
        15: Config.UNMAPPED_STATION_ID, // Midtown not mapped and does not have long/lat in old mapping
        16: "101",
        17: "109",
        18: "108",
        19: "9218",
        20: Config.UNMAPPED_STATION_ID, // South Junction not mapped and does not have long/lat in old mapping
        21: "107",
        22: "97",
        23: "112",
        24: "114",
        26: "118",
        27: "119",
        28: "120",
        29: "123",
        30: "124",
        31: "121",
        32: "106",
        33: "98",
        34: "99",
        35: "100",
        36: "130",
        37: "103",
        38: Config.UNMAPPED_STATION_ID, // Harbour not mapped and does not have long/lat in old mapping
        39: "113",
        40: "134",
        41: "277",
        115: "126",
        202: "116",
        371: "117",
        2000: Config.UNMAPPED_STATION_ID, // Park Junction not mapped and does not have long/lat in old mapping
        2001: "606",
        2002: "140", // Hillcrest slight diff in long/lat
        2004: Config.UNMAPPED_STATION_ID, // West Junction not mapped and does not have long/lat in old mapping
        2005: Config.UNMAPPED_STATION_ID, // Depot not mapped and does not have long/lat in old mapping
    };

    public static apm = {
        packageVersion: process.env.npm_package_version,
        otelEndpoint: process.env.OTEL_ENDPOINT,
        otelToken: process.env.OTEL_TOKEN,
        appName: process.env.APP_NAME,
        nodeEnv: process.env.NODE_ENV,
        framework: "express",
    };

    private static dormantSuffix = process.env.IS_DORMANT === "true" ? ".dormant" : "";
    public static kafka = {
        consumer: {
            endpoints: (process.env.KAFKA_HOSTS || "kafka.transit.example.com:9092").split(","),
            topics: (Config.addConfluentPrefix(process.env.KAFKA_SOURCE_TOPIC) || "dev-realtime-train.v1").split(","),
            consumerGroup: Config.addConfluentPrefix(process.env.KAFKA_CONSUMER_GROUP || "train-position-adapter-v2") + this.dormantSuffix,
        },
        producer: {
            endpoints: (process.env.KAFKA_HOSTS || "kafka.transit.example.com:9092").split(","),
            vpTopic: Config.addConfluentPrefix(process.env.KAFKA_DEST_VP_TOPIC || "realtime-train-positions") + this.dormantSuffix + ".v1",
        },
    };

    public static getAzureAccessTokenRetrieverConfig(): AzureAccessTokenRetrieverConfig {
        return {
            clientId: process.env.APP_MANIFEST_CLIENT_ID || "00000000-0000-0000-0000-000000000001",
            domain: "transit.example.com",
            keyVault: {
                host: `https://${process.env.KEY_VAULT || "kv-realtime-dev"}.vault.azure.net`,
                secretNameSystemClientSecret: process.env.KEY_VAULT_SECRET_NAME_SYSTEM_CLIENT_SECRET || "system-client-secret",
            },
            loggerConfig: {
                logger: Config.logger as never,
                logAsInfo: process.env.AUTH_LOGGING === "true",
            },
            localDevEnv: {
                accessToken: process.env.LOCAL_ACCESS_TOKEN,
            },
        };
    }

    public static addConfluentPrefix(value: string | undefined): string | undefined {
        if (!value) {
            return value;
        }
        return `${process.env.KAFKA_ENVIRONMENT_PREFIX}${value}`;
    }
}
