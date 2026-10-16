import { Config } from "./config";
import { KeyVault } from "./key-vault";

export class ConfluentSecretRetriever {
    private static confluentSecret = "";

    public static confluentKafkaSecret = {
        username: "",
        password: "",
    };

    public static async retrieve(): Promise<void> {
        if (this.confluentKafkaSecret.username && this.confluentKafkaSecret.password) {
            return;
        }

        const kv = new KeyVault(Config.keyVaultHost);

        this.confluentSecret = await kv.getSecret(Config.brokerSecretName);
        if (!this.confluentSecret) {
            throw new Error("No valid kafka keys");
        }
        const { key, secret } = JSON.parse(this.confluentSecret);
        this.confluentKafkaSecret.username = key;
        this.confluentKafkaSecret.password = secret;
    }

    // the producer cannot be rebuilt in place, so a changed key restarts the
    // process and the next start reads the new one
    public static async watch(closeApp: () => void): Promise<void> {
        const kv = new KeyVault(Config.keyVaultHost);
        const latest = await kv.getSecret(Config.brokerSecretName);
        if (latest && latest !== this.confluentSecret) {
            closeApp();
        }

        setTimeout(() => this.watch(closeApp), Config.secretWatcherMinutes * 60 * 1000);
    }
}
