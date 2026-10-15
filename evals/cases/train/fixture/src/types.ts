export class ValidationError extends Error {
    constructor(public errorType: string, msg: string) {
        super(msg);
        Object.setPrototypeOf(this, ValidationError.prototype);
    }
}
