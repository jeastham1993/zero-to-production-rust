import { RustFunction } from '@cdklabs/aws-lambda-rust';
import { ITable } from 'aws-cdk-lib/aws-dynamodb';
import { Role, ServicePrincipal } from 'aws-cdk-lib/aws-iam';
import { Architecture, DockerImageCode, DockerImageFunction } from 'aws-cdk-lib/aws-lambda';
import { SqsEventSource } from 'aws-cdk-lib/aws-lambda-event-sources';
import { CfnPipe } from 'aws-cdk-lib/aws-pipes';
import { IBucket } from 'aws-cdk-lib/aws-s3';
import { Queue } from 'aws-cdk-lib/aws-sqs';
import { StringParameter } from 'aws-cdk-lib/aws-ssm';
import { Construct } from 'constructs';

export interface NewSubscriberProcessingStackProps {
    newsletterTable: ITable,
    newsletterStorageBucket: IBucket,
    configParameter: StringParameter
}

export class NewSubscriberProcessingStack extends Construct {

  constructor(scope: Construct, id: string, props: NewSubscriberProcessingStackProps) {
    super(scope, id);

    const new_subscriber_queue = new Queue(this, "NewSubscriberQueue");

    var pipeRole = new Role(this, "PipeIntegrationRole", {
        assumedBy: new ServicePrincipal("pipes.amazonaws.com")
      });
  
      props.newsletterTable.grantStreamRead(pipeRole);
      new_subscriber_queue.grantSendMessages(pipeRole);
  
      var pipe = new CfnPipe(this, "NewSubscriberPipe", {
        roleArn: pipeRole.roleArn,
        source: (props.newsletterTable.tableStreamArn ?? ""),
        sourceParameters: {
          dynamoDbStreamParameters: {
            startingPosition: "TRIM_HORIZON",
            batchSize: 10
          },
          filterCriteria: {
            filters: [{
              pattern: '{"dynamodb.NewImage.Type.S": ["SubscriberToken"]}'
            }]
          },
        },
        target: new_subscriber_queue.queueArn,
        targetParameters: {
          sqsQueueParameters:{
  
          },
          inputTemplate: `{
            "trace_parent": <$.dynamodb.NewImage.TraceParent.S>,
            "parent_span": <$.dynamodb.NewImage.ParentSpan.S>,
            "email_address": <$.dynamodb.NewImage.EmailAddress.S>,
            "subscriber_token": <$.dynamodb.NewImage.PK.S>
          }`
        }
      })
  
      const send_confirmation_function = new RustFunction(this, "ConfirmationFunction", {
        entry: '../src/backend/Cargo.toml',
        binaryName: 'send_confirmation',
        architecture: Architecture.ARM_64,
        environment: {
          LOG_LEVEL: "error",
          CONFIG_PARAMETER_NAME: props.configParameter.parameterName,
          APP_ENVIRONMENT: "production"
        },
      });
  
      send_confirmation_function.addEventSource(new SqsEventSource(new_subscriber_queue, {
        batchSize: 10
      }));

      props.configParameter.grantRead(send_confirmation_function);
}
}
