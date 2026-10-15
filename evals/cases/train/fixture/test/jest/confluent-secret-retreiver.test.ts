import { ConfluentSecretRetriever } from "../../src/confluent-secret-retriever";
import { KeyVault } from "../../src/key-vault";

describe("Test loading configuration", () => {

    beforeEach(async () => {
        jest.clearAllMocks();
        ConfluentSecretRetriever.confluentKafkaSecret = { username: "", password: "" };
    });

    it("when kafka secret is not retrieved, should throw error for invalid kafka key", async () => {
        jest.spyOn(KeyVault.prototype, "getSecret").mockImplementation(() => Promise.resolve(""));
        await expect(
            ConfluentSecretRetriever.retrieve(),
        ).rejects.toThrow("No valid kafka keys");
    });

    it("when kafka secret is retrieved, should get correct confluent kafka username and password", async () => {
        const validSecret = { key: "ABCDEF", secret: "my secret" };
        const getSecret = jest.spyOn(KeyVault.prototype, "getSecret").mockImplementation(() => Promise.resolve(JSON.stringify(validSecret)));
        await ConfluentSecretRetriever.retrieve();

        expect(getSecret).toHaveBeenCalledWith("realtime-confluent-key");
        expect(ConfluentSecretRetriever.confluentKafkaSecret.username).toEqual(validSecret.key);
        expect(ConfluentSecretRetriever.confluentKafkaSecret.password).toEqual(validSecret.secret);
    });

    it("when has local dev, should not call get secret", async () => {
        ConfluentSecretRetriever.confluentKafkaSecret = { username: "ABCDEF", password: "my secret" };

        const getSecret = jest.spyOn(KeyVault.prototype, "getSecret").mockImplementation(() => Promise.resolve(""));

        await ConfluentSecretRetriever.retrieve();

        expect(getSecret).not.toHaveBeenCalled();
    });

    it("when kafka secret is changed, should close app", async () => {
        const initSecret = { key: "ABCDEF", secret: "my secret" };
        const updatedSecret = { key: "FEDCBA", secret: "my secret 2" };

        const getSecret = jest
            .spyOn(KeyVault.prototype, "getSecret")
            .mockImplementation(() => Promise.resolve(JSON.stringify(initSecret)))
            .mockImplementationOnce(() => Promise.resolve(JSON.stringify(initSecret)))
            .mockImplementationOnce(() => Promise.resolve(JSON.stringify(updatedSecret)));

        const closeApp = jest.fn();
        await ConfluentSecretRetriever.retrieve();
        await ConfluentSecretRetriever.watch(() => {
            closeApp();
        });

        expect(getSecret).toHaveBeenCalledTimes(2);
        expect(getSecret).toHaveBeenCalledWith("realtime-confluent-key");
        expect(closeApp).toBeCalled();
    });
});
