import { Config } from "../config";
import { ApmCommon, AxiosCommon, UtilsCommon } from "realtime-common";

const { APMService } = ApmCommon;

export class GtfsApi {
    private stops = [];

    constructor() {
        this.scheduleStopsFetch();
    }

    public getStopInfoByStopCode(stopCode: string): { stop_code: string, stop_lat: number, stop_lon: number } | undefined {
        return this.stops.find((stop: { stop_code: string, stop_lat: number, stop_lon: number }) => stop.stop_code === stopCode);
    }

    public async fetchStops() {
        const url = `${Config.staticCCUrl}/gtfs/stops?fields=stop_code,stop_lon,stop_lat`;
        const staticCCApiRequest = async () => await Config.axios.get(url);

        const response = await UtilsCommon.retry(staticCCApiRequest, {
            onRetry: async (err: AxiosCommon.AxiosError, attempt: number) => {
                if (err.response?.status && err.response.status < 500) {
                    throw err;
                }
                Config.logger.warn(`(retrying ${attempt}) ${url} due to: ${err.stack || err.message}`);
            },
        }).catch((err) => {
            APMService.noticeError(err);
            Config.logger.error(`Could not fetch stops from "${url}" due to: ${err.stack || err.message}`);
        });

        if (response && response.data && response.data.length) {
            this.stops = response.data;
        }
    }

    private scheduleStopsFetch() {
        setTimeout(async () => {
            await this.fetchStops();
            this.scheduleStopsFetch();
        }, 60 * 60 * 1000);
    }
}
