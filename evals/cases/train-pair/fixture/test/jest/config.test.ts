import { Config } from "../../src/config";

jest.mock("@azure/keyvault-secrets", () => ({
    SecretClient: jest.fn(() => ({
        getSecret: async () => ({
            value: JSON.stringify({
                key: "test",
                secret: "password",
            }),
        }),
    })),
}));

describe("Test APM config", () => {
    it("Should expose apm config block with all OTel properties", () => {
        expect(Config.apm).toHaveProperty("packageVersion");
        expect(Config.apm).toHaveProperty("otelEndpoint");
        expect(Config.apm).toHaveProperty("otelToken");
        expect(Config.apm).toHaveProperty("appName");
        expect(Config.apm).toHaveProperty("nodeEnv");
    });

    it("Should set apm.appName from Config.appName", () => {
        expect(Config.apm.appName).toBe(Config.appName);
    });

    it("Should have undefined OTel fields when env vars are not set", () => {
        expect(Config.apm.otelEndpoint).toBeUndefined();
        expect(Config.apm.otelToken).toBeUndefined();
    });
});

describe("Test Kafka Configs", () => {
    it("When isLocal is false, should get confluent configs", async () => {
        process.env.KAFKA_HOSTS = "kafka.transit.example.com:9092";
        Config.useConfluentKafkaConfig = true;
        const result = await Config.getKafkaProducerConfig();
        expect(result).toStrictEqual({
            producer: {
                endpoint: ["kafka.transit.example.com:9092"],
                topic: "dev-realtime-train.v1",
                sasl: {
                    mechanism: "plain",
                    password: "password",
                    username: "test",
                },
                ssl: true,
            },
        });
    });

    it("When isLocal is true, should get local configs", async () => {
        process.env.KAFKA_HOSTS = "kafka.transit.example.com:9092";
        Config.useConfluentKafkaConfig = false;
        const result = await Config.getKafkaProducerConfig();
        expect(result).toStrictEqual({
            producer: {
                endpoint: ["kafka.transit.example.com:9092"],
                topic: "dev-realtime-train.v1",
                sasl: undefined,
                ssl: false,
            },
        });
    });

    it("When dormant, should add dormant infix to producer topic", async () => {
        process.env.IS_DORMANT = "true";
        process.env.KAFKA_ENVIRONMENT = "test";
        process.env.KAFKA_HOSTS = "kafka.transit.example.com:9092";
        Config.useConfluentKafkaConfig = false;

        const result = await Config.getKafkaProducerConfig();

        expect(result.producer.topic).toBe("tst-realtime-train.dormant.v1");

        delete process.env.IS_DORMANT;
        delete process.env.KAFKA_ENVIRONMENT;
    });
});
