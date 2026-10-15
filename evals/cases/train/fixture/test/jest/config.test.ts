describe("Config", () => {

    beforeEach(() => {
        jest.resetModules();
    });

    it("should have correct properties for Kafka and returning confluent variables", () => {
        process.env = {
            KAFKA_ENVIRONMENT_PREFIX: "dev-",
            KAFKA_SOURCE_TOPIC: "realtime-train.v1",
            KAFKA_CONSUMER_GROUP: "train-position-adapter-v2",
            KAFKA_DEST_VP_TOPIC: "realtime-train-positions",
        };

        const { Config } = require("../../src/config");
        const { consumer, producer } = Config.kafka;

        expect(consumer.endpoints).toEqual(["kafka.transit.example.com:9092"]);
        expect(consumer.topics).toEqual(["dev-realtime-train.v1"]);
        expect(consumer.consumerGroup).toEqual("dev-train-position-adapter-v2");

        expect(producer.endpoints).toEqual(["kafka.transit.example.com:9092"]);
        expect(producer.vpTopic).toEqual("dev-realtime-train-positions.v1");
    });

    it("should have correct properties for production Kafka and returning confluent variables", () => {
        process.env = {
            KAFKA_ENVIRONMENT_PREFIX: "",
            KAFKA_SOURCE_TOPIC: "realtime-train.v1",
            KAFKA_CONSUMER_GROUP: "train-position-adapter-v2",
            KAFKA_DEST_VP_TOPIC: "realtime-train-positions",
        };

        const { Config } = require("../../src/config");
        const { consumer, producer } = Config.kafka;

        expect(consumer.endpoints).toEqual(["kafka.transit.example.com:9092"]);
        expect(consumer.topics).toEqual(["realtime-train.v1"]);
        expect(consumer.consumerGroup).toEqual("train-position-adapter-v2");

        expect(producer.endpoints).toEqual(["kafka.transit.example.com:9092"]);
        expect(producer.vpTopic).toEqual("realtime-train-positions.v1");
    });

    it("should have correct properties for Kafka when dormant and returning confluent variables", () => {
        process.env = {
            IS_DORMANT: "true",
            KAFKA_ENVIRONMENT_PREFIX: "tst-",
            KAFKA_SOURCE_TOPIC: "realtime-train.v1",
            KAFKA_CONSUMER_GROUP: "train-position-adapter-v2",
            KAFKA_DEST_VP_TOPIC: "realtime-train-positions",
        };

        const { Config } = require("../../src/config");
        const { consumer, producer } = Config.kafka;

        expect(consumer.endpoints).toEqual(["kafka.transit.example.com:9092"]);
        expect(consumer.topics).toEqual(["tst-realtime-train.v1"]);
        expect(consumer.consumerGroup).toEqual("tst-train-position-adapter-v2.dormant");

        expect(producer.endpoints).toEqual(["kafka.transit.example.com:9092"]);
        expect(producer.vpTopic).toEqual("tst-realtime-train-positions.dormant.v1");
    });
});
