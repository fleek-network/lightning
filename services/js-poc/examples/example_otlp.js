import { metrics, trace } from 'https://esm.sh/v135/@opentelemetry/api@1.9.0/es2022/api.mjs#integrity=sha256-FJtOdNSoYYZNmPubXHWEYUHjQl0wzfcQAfSdY9fIeCw=';

const tracer = trace.getTracer("my-app", "1.0.0");
const meter = metrics.getMeter("my-app", "1.0.0");
const counter = meter.createCounter("my_counter", {
  description: "A simple counter",
  unit: "1",
});

export const main = () => {
  // normal console logs are captured and exported
  console.log("normal log foobar");

  tracer.startActiveSpan("myFunction", (span) => {
    try {
      // do some work
      console.log("inside span");
      counter.add(1);
    } catch (error) {
      // error handling
      span.recordException(error);
      span.setStatus({
        code: trace.SpanStatusCode.ERROR,
        message: error.message,
      });
      throw error;
    } finally {
      // end span
      span.end();
    }
  });

  // meter metrics
  counter.add(1);
};
