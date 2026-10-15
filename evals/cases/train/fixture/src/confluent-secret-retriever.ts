import { KeyVault } from "./key-vault";

export class ConfluentSecretRetriever {
    private static confluentKeyName = process.env.KEY_VAULT_SECRET_NAME_CONFLUENT_KEY || "realtime-confluent-key";
    private static keyVault = "https://" + process.env.KEY_VAULT + ".vault.azure.net";
    private static secretWatcherInterval = Number(process.env.SECRET_WATCHER_INTERVAL) || 5;
    private static confluentSecret = "";

    public static confluentKafkaSecret = {
        username: "",
        password: "",
    };

    public static async retrieve(): Promise<void> {
        if (this.confluentKafkaSecret.username && this.confluentKafkaSecret.password) {
            return;
        }

        const kv = new KeyVault(this.keyVault);

        this.confluentSecret = await kv.getSecret(this.confluentKeyName);
        if (!this.confluentSecret) {
            throw new Error("No valid kafka keys");
        }
        const { key, secret } = JSON.parse(this.confluentSecret);
        this.confluentKafkaSecret.username = key;
        this.confluentKafkaSecret.password = secret;

    }

    // we use simple restart mechanism as legacy consumer does not support rebuilding out of the box
    // and restart of the application should pick up latest version of the key without the problem
    public static async watch(closeApp: () => void): Promise<void> {
        const kv = new KeyVault(this.keyVault);
        const kafkaLatestSecret = await kv.getSecret(this.confluentKeyName);
        if (kafkaLatestSecret && kafkaLatestSecret !== this.confluentSecret) {
            closeApp();
        }

        setTimeout(() => this.watch(closeApp), this.secretWatcherInterval * 60 * 1000);
    }
}
