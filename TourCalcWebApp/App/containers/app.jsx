import React from 'react';
import ReactDOM from 'react-dom';
import { BrowserRouter as Router, Route, Switch } from 'react-router-dom';
import Header from './header/header.jsx';
import TourPersons from './tour/persons.jsx';
import TourSpendings from './tour/spendings.jsx';
import TourChoose from './tour/choose.jsx';
import Auth from './auth/auth.jsx';
import { Redirect } from 'react-router-dom'
import FetchHelper from './tour/helpers.jsx'

export default class App extends React.Component {
    constructor(props) {
        super(props);
        this.chooseTour = this.chooseTour.bind(this);
        this.state = {
            isTourChosen: false,
            chosenTour: null,
            error: null,
            isLoaded: false,
            tours: []
        };
    }

    componentDidMount() {
        FetchHelper.fetchTours(this);
    }


    chooseTour(tour) {
        this.setState({
            isTourChosen: true,
            chosenTour: tour
        })
    }
    render() {
        const { isTourChosen, chosenTour, error, isLoaded, tours } = this.state;
        if (error) {
            return <div>Error: {error.message}</div>;
        } else if (!isLoaded) {
            return <div>Loading...</div>;
        } else {
            if (this.state.chosenTour == null) {
                if (tours.length > 0) {
                    this.state.chosenTour = tours[0]
                    this.state.isTourChosen = true
                }
            }
            return (
                <Router>
                    <div>
                        <Header tour={this.state.chosenTour} />
                        <main>
                            <Switch>
                                <Route path="/spendings"
                                    render={(props) => <TourSpendings
                                        tourid={this.state.isTourChosen ? this.state.chosenTour.id : null}
                                        />} />
                                <Route path="/auth" component={Auth} />
                                <Route path="/persons"
                                    render={(props) => <TourPersons
                                        tourid={this.state.isTourChosen ? this.state.chosenTour.id : null}
                                        />} />
                                <Route path="/choose"
                                    render={(props) => <TourChoose
                                        chosenTour={this.state.chosenTour}
                                        chooseTourAction={this.chooseTour}
                                    />} />
                                <Route path="/"
                                    render={(props) => <Redirect to="/choose" />} />
                            </Switch>
                        </main>
                    </div>
                </Router>
            );
        }
    }
};
ReactDOM.render(
    <App />,
    document.getElementById("content")
);