import { SecretClient } from "@azure/keyvault-secrets";
import { DefaultAzureCredential } from "@azure/identity";
export class KeyVault {
    constructor(private kvHost: string) { }

    public async getSecret(secretName: string): Promise<string> {
        const secret = await new SecretClient(this.kvHost, new DefaultAzureCredential()).getSecret(secretName);

        if (!secret?.value) {
            throw new Error(`Could not find secret under secretName=${secretName} from Azure Key Vault`);
        }

        return secret.value;
    }
}
