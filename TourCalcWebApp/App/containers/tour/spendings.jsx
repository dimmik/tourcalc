import React from 'react';

export default class TourSpendings extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            error: null,
            isLoaded: false,
            tour: {}
        };
    }

    componentDidMount() {
        if (this.props.tourid != null) {
            fetch('/api/tour/' + this.props.tourid + '/calculated')
                .then(res => res.json())
                .then(
                    (result) => {
                        this.setState({
                            isLoaded: true,
                            tour: result
                        });
                    },
                    (error) => {
                        this.setState({
                            isLoaded: true,
                            error
                        });
                    })
        }
    }

    render() {
        if (this.props.tourid == null) return (<div>please choose tour</div>);

        const { error, isLoaded, tour } = this.state;
        if (error) {
            return (<h1>error: {error.message}</h1>)
        } else if (!isLoaded) {
            return (<div>Loading...</div>)
        } else {
            return (
                <div>
                    <ol>
                        {
                            tour.spendings.map(
                                (sp) => {
                                    const person = tour.persons.find((p) => p.guid == sp.fromGuid);
                                    const toP = sp.toGuid.map((guid) => tour.persons.find((p) => p.guid == guid));
                                    return (
                                        <li key={sp.guid}>'{sp.description}': {sp.amountInCents / 100}
                                            <br />from: {person != null ? person.name : "unknown"} -- {sp.fromGuid}
                                            <br />toAll: {sp.toAll ? 'yes' : 'no'}
                                            <br />to: <ul> {sp.toAll ? 'n/a' : toP.map(p => <li key={p.guid}>{p.name},</li>)} </ul>
                                        </li>
                                    )
                                }
                            )

                        }
                    </ol>
                </div>
            )
        }

    }
};