export const OK_RESPONSE = {
    return: "OK",
};

export const ERROR_RESPONSE = {
    Fault: {
        statusCode: 500,
        response: {
            message: "Internal Server Error",
        },
    },
};

export const NOT_IMPLEMENTED = {
    Fault: {
        statusCode: 501,
        response: {
            message: "Method Not Implemented",
        },
    },
};
