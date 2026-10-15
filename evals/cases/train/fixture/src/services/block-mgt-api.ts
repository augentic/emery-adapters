import { Config } from "../config";
import { ApmCommon, UtilsCommon, AxiosCommon } from "realtime-common";
import { AzureSystemAccessTokenRetriever } from "auth-common";

const { APMService } = ApmCommon;

export class BlockMgtApi {
    private authTokenRetriever: AzureSystemAccessTokenRetriever;

    public async getVehiclesByExternalRefId(externalRefId: string): Promise<string[]> {
        const url = `${Config.blockMgtUrl}/allocations/trips?externalRefId=${externalRefId}&closestTrip=true&include_filtered_routes=true`;
        const blockMgtRequest = async () => await Config.axios
            .get(url, {
                headers: {
                    Authorization: `Bearer ${await this.authTokenRetriever.retrieve()}`,
                },
            });

        const response = await UtilsCommon.retry(blockMgtRequest, {
            onRetry: async (err: AxiosCommon.AxiosError, attempt: number) => {
                if (err.response?.status && err.response.status < 500) {
                    throw err;
                }
                Config.logger.warn(`(retrying ${attempt}) ${url} due to: ${err.stack || err.message}`);
            },
        }).catch((err) => {
            APMService.noticeError(err);
            Config.logger.error(`Could not fetch allocation from "${url}" due to: ${err.stack || err.message}`);
        });

        return (response || {})?.data?.all?.map((allocation: { vehicleLabel: string }) => allocation.vehicleLabel) || [];
    }

    public async fetchAuthToken() {
        this.authTokenRetriever = await AzureSystemAccessTokenRetriever.provide(Config.getAzureAccessTokenRetrieverConfig());
    }
}
