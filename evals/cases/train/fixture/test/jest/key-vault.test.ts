import { KeyVault } from "../../src/key-vault";
import { SecretClient, KeyVaultSecret } from "@azure/keyvault-secrets";

describe("Test loading configuration", () => {
    it("When load secret from key vault with no issue, should return secret value", async () => {
        const secretValue = '{ key: "ABCDEF", secret: "my secret" }';
        const secret = {
            value: secretValue,
            name: "test",
            properties: {
                vaultUrl: "",
                name: "",
            },
        };
        const getSecret = jest.spyOn(SecretClient.prototype, "getSecret").mockReturnValue(Promise.resolve(secret));
        const kv = new KeyVault("https://test.vault.azure.net");
        const keyName = "test-key";
        const result = await kv.getSecret(keyName);
        expect(getSecret).toHaveBeenCalledWith(keyName);
        expect(result).toEqual(secretValue);
    });

    it("When load secret is empty, should throw error", async () => {
        const secret = {
            value: "",
            name: "test",
            properties: {
                vaultUrl: "",
                name: "",
            },
        };
        jest.spyOn(SecretClient.prototype, "getSecret").mockReturnValue(Promise.resolve(secret));
        const kv = new KeyVault("https://test.vault.azure.net");
        const keyName = "test-key";
        await expect(
            kv.getSecret(keyName)
        ).rejects.toThrow("Could not find secret under secretName=test-key from Azure Key Vault");
    });

    it("When load secret is null, should throw error", async () => {
        const secret = null as unknown as KeyVaultSecret;
        jest.spyOn(SecretClient.prototype, "getSecret").mockReturnValue(Promise.resolve(secret));
        const kv = new KeyVault("https://test.vault.azure.net");
        const keyName = "test-key";
        await expect(
            kv.getSecret(keyName)
        ).rejects.toThrow("Could not find secret under secretName=test-key from Azure Key Vault");
    });
});
